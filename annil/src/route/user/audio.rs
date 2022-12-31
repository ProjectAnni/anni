use crate::error::AnnilError;
use crate::extractor::token::AnnilClaim;
use crate::extractor::track::TrackIdentifier;
use crate::state::AnnilProviders;
use crate::utils::Either;
use anni_provider::Range;
use axum::body::StreamBody;
use axum::extract::Query;
use axum::http::header::{
    ACCEPT_RANGES, ACCESS_CONTROL_EXPOSE_HEADERS, CACHE_CONTROL, CONTENT_LENGTH, CONTENT_RANGE,
    CONTENT_TYPE,
};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{AppendHeaders, IntoResponse, Response};
use axum::Extension;
use futures::StreamExt;
use serde::Deserialize;
use std::process::Stdio;
use std::sync::Arc;
use tokio_util::io::ReaderStream;

#[derive(Deserialize)]
pub struct AudioQuery {
    #[serde(rename = "quality")]
    quality_requested: Option<String>,
}

impl AudioQuery {
    pub fn quality(&self, is_guest: bool) -> &str {
        match (&self.quality_requested.as_deref(), is_guest) {
            (Some("low"), false) => "low",
            (Some("high"), false) => "high",
            (Some("lossless"), false) => "lossless",
            _ => "medium",
        }
    }

    pub fn need_transcode(&self, is_guest: bool) -> bool {
        self.quality(is_guest) != "lossless"
    }
}

pub async fn audio_head(
    claim: AnnilClaim,
    track: TrackIdentifier,
    Extension(providers): Extension<Arc<AnnilProviders>>,
    query: Query<AudioQuery>,
) -> Response {
    if !claim.can_fetch(&track) {
        return AnnilError::Unauthorized.into_response();
    }

    for provider in providers.read().await.iter() {
        if provider.has_album(&track.album_id).await {
            let audio = provider
                .get_audio_info(&track.album_id.to_string(), track.disc_id, track.track_id)
                .await
                .map_err(|_| AnnilError::NotFound);
            let transcode = query.need_transcode(claim.is_guest());
            return match audio {
                Ok(info) => {
                    let headers = [
                        (
                            CONTENT_TYPE,
                            if transcode {
                                "audio/aac".to_string()
                            } else {
                                format!("audio/{}", info.extension)
                            },
                        ),
                        (
                            ACCESS_CONTROL_EXPOSE_HEADERS,
                            "X-Origin-Type, X-Origin-Size, X-Duration-Seconds, X-Audio-Quality, Accept-Ranges".to_string(),
                        ),
                    ];
                    let custom_headers = [
                        ("X-Origin-Type", format!("audio/{}", info.extension)),
                        ("X-Origin-Size", format!("{}", info.size)),
                        ("X-Duration-Seconds", format!("{}", info.duration)),
                        (
                            "X-Audio-Quality",
                            query.quality(claim.is_guest()).to_string(),
                        ),
                    ];

                    let transcode_headers = if !transcode {
                        Either::Left([
                            (ACCEPT_RANGES, "bytes".to_string()),
                            (CONTENT_LENGTH, format!("{}", info.size)),
                        ])
                    } else {
                        Either::Right(())
                    };

                    (headers, custom_headers, transcode_headers).into_response()
                }
                Err(e) => e.into_response(),
            };
        }
    }

    (
        StatusCode::NOT_FOUND,
        AppendHeaders([(CACHE_CONTROL, "private")]),
    )
        .into_response()
}

/// Get audio in an album with `album_id`, `disc_id` and `track_id`
pub async fn audio(
    claim: AnnilClaim,
    track: TrackIdentifier,
    Extension(providers): Extension<Arc<AnnilProviders>>,
    query: Query<AudioQuery>,
    headers: HeaderMap,
) -> Response {
    if !claim.can_fetch(&track) {
        return AnnilError::Unauthorized.into_response();
    }

    let bitrate = match query.quality(claim.is_guest()) {
        "low" => Some("128k"),
        "medium" => Some("192k"),
        "high" => Some("320k"),
        "lossless" => None,
        _ => Some("128k"),
    };
    let range = headers.get("Range").and_then(|r| {
        let range = r.to_str().ok()?;
        let (_, right) = range.split_once('=')?;
        let (from, to) = right.split_once('-')?;
        let range = Range::new(from.parse().ok()?, to.parse().ok());
        Some(if range.is_full() {
            Range::new(0, Some(1023))
        } else {
            range
        })
    });
    let need_range = range.is_some();
    let range = range.unwrap_or(Range::FULL);

    for provider in providers.read().await.iter() {
        if provider.has_album(&track.album_id).await {
            // range is only supported on lossless
            let range = if bitrate.is_some() {
                Range::FULL
            } else {
                range
            };
            let audio = provider
                .get_audio(
                    &track.album_id.to_string(),
                    track.disc_id,
                    track.track_id,
                    range,
                )
                .await
                .map_err(|_| AnnilError::NotFound);

            return match audio {
                Ok(mut audio) => {
                    let (status, range) = if need_range && !audio.range.is_full() {
                        (
                            StatusCode::PARTIAL_CONTENT,
                            Some([
                                (CONTENT_RANGE, audio.range.to_content_range_header()),
                                (ACCEPT_RANGES, "bytes".to_string()),
                            ]),
                        )
                    } else {
                        (StatusCode::OK, None)
                    };

                    let header = [(
                        ACCESS_CONTROL_EXPOSE_HEADERS,
                        "X-Origin-Type, X-Origin-Size, X-Duration-Seconds, X-Audio-Quality"
                            .to_string(),
                    )];

                    let headers = [
                        ("X-Origin-Type", format!("audio/{}", audio.info.extension)),
                        ("X-Origin-Size", format!("{}", audio.info.size)),
                        ("X-Duration-Seconds", format!("{}", audio.info.duration)),
                        (
                            "X-Audio-Quality",
                            query.quality(claim.is_guest()).to_string(),
                        ),
                    ];

                    let body = match bitrate {
                        Some(bitrate) => {
                            let mut process = tokio::process::Command::new("ffmpeg")
                                .args(&[
                                    "-i", "pipe:0", "-map", "0:0", "-b:a", bitrate, "-f", "adts",
                                    "-",
                                ])
                                .stdin(Stdio::piped())
                                .stdout(Stdio::piped())
                                .stderr(Stdio::null())
                                .spawn()
                                .unwrap();
                            let stdout = process.stdout.take().unwrap();
                            tokio::spawn(async move {
                                let mut stdin = process.stdin.as_mut().unwrap();
                                let _ = tokio::io::copy(&mut audio.reader, &mut stdin).await;
                            });
                            Either::Left((
                                [(CONTENT_TYPE, "audio/aac")],
                                StreamBody::new(ReaderStream::new(stdout)),
                            ))
                        }
                        None => {
                            let size = audio.range.length().unwrap_or(audio.info.size as u64);
                            Either::Right((
                                [
                                    (CONTENT_LENGTH, format!("{size}")),
                                    (CONTENT_TYPE, format!("audio/{}", audio.info.extension)),
                                ],
                                StreamBody::new(
                                    ReaderStream::new(audio.reader).take(size as usize),
                                ),
                            ))
                        }
                    };

                    (status, range, header, headers, body).into_response()
                }
                Err(e) => e.into_response(), // TODO: continue to retry remaining providers
            };
        }
    }

    (StatusCode::NOT_FOUND, [(CACHE_CONTROL, "private")]).into_response()
}
