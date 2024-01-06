use crate::error::AnnilError;
use crate::extractor::token::AnnilClaim;
use crate::extractor::track::TrackIdentifier;
use crate::provider::AnnilProvider;
use crate::transcode::*;
use crate::utils::Either;
use anni_provider::{AnniProvider, Range};
use axum::body::Body;
use axum::extract::Query;
use axum::http::header::{
    ACCEPT_RANGES, ACCESS_CONTROL_EXPOSE_HEADERS, CACHE_CONTROL, CONTENT_LENGTH, CONTENT_RANGE,
    CONTENT_TYPE,
};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Extension;
use futures::StreamExt;
use serde::Deserialize;
use std::str::FromStr;
use std::sync::Arc;
use tokio_util::io::ReaderStream;

#[derive(Copy, Clone)]
pub enum AudioQuality {
    Low,
    Medium,
    High,
    Lossless,
}

impl AudioQuality {
    /// Transcode if the target quality is not lossless
    pub fn need_transcode(&self) -> bool {
        if let AudioQuality::Lossless = self {
            false
        } else {
            true
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            AudioQuality::Low => "low",
            AudioQuality::Medium => "medium",
            AudioQuality::High => "high",
            AudioQuality::Lossless => "lossless",
        }
    }
}

impl FromStr for AudioQuality {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "low" => Ok(AudioQuality::Low),
            "high" => Ok(AudioQuality::High),
            "lossless" => Ok(AudioQuality::Lossless),
            _ => Ok(AudioQuality::Medium),
        }
    }
}

#[derive(Deserialize)]
pub struct AudioQuery {
    #[serde(rename = "quality")]
    quality_requested: Option<String>,

    #[serde(default)]
    opus: bool,
}

impl AudioQuery {
    pub fn get_transcoder(&self, is_guest: bool) -> Box<dyn Transcode + Send + Sync> {
        let quality = self.quality(is_guest);
        if quality.need_transcode() {
            if self.opus {
                Box::new(OpusTranscoder::new(quality))
            } else {
                Box::new(AacTranscoder::new(quality))
            }
        } else {
            Box::new(FlacTranscoder::new(quality))
        }
    }

    fn quality(&self, is_guest: bool) -> AudioQuality {
        if is_guest {
            return AudioQuality::Low;
        }
        AudioQuality::from_str(self.quality_requested.as_deref().unwrap_or("medium")).unwrap()
    }
}

pub async fn audio_head<P>(
    claim: AnnilClaim,
    track: TrackIdentifier,
    Extension(provider): Extension<Arc<AnnilProvider<P>>>,
    query: Query<AudioQuery>,
) -> Response
where
    P: AnniProvider + Send + Sync,
{
    if !claim.can_fetch(&track) {
        return AnnilError::Unauthorized.into_response();
    }

    let provider = provider.read().await;
    let album_id = track.album_id.to_string();
    if !provider.has_album(&album_id).await {
        return (StatusCode::NOT_FOUND, [(CACHE_CONTROL, "private")]).into_response();
    }

    let audio = provider
        .get_audio_info(&album_id, track.disc_id, track.track_id)
        .await
        .map_err(|_| AnnilError::NotFound);

    let transcoder = query.get_transcoder(claim.is_guest());
    let need_transcode = transcoder.need_transcode();

    return match audio {
        Ok(info) => {
            let headers = [
                        (
                            CONTENT_TYPE,
                            if need_transcode {
                                transcoder.content_type().to_string()
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
                ("X-Duration-Seconds", format!("{}", info.duration / 1000)),
                (
                    "X-Audio-Quality",
                    query.quality(claim.is_guest()).as_str().to_string(),
                ),
            ];

            let mut transcode_headers = HeaderMap::new();

            if let Some(length) = transcoder.content_length(&info) {
                transcode_headers.insert(CONTENT_LENGTH, length.into());
            }

            // TODO: support range for all formats with CONTENT_LENGTH
            if !need_transcode {
                transcode_headers.insert(ACCEPT_RANGES, "bytes".parse().unwrap());
            }

            (headers, custom_headers, transcode_headers).into_response()
        }
        Err(e) => e.into_response(),
    };
}

/// Get audio in an album with `album_id`, `disc_id` and `track_id`
pub async fn audio<P>(
    claim: AnnilClaim,
    track: TrackIdentifier,
    Extension(provider): Extension<Arc<AnnilProvider<P>>>,
    query: Query<AudioQuery>,
    headers: HeaderMap,
) -> Response
where
    P: AnniProvider + Send + Sync,
{
    if !claim.can_fetch(&track) {
        return AnnilError::Unauthorized.into_response();
    }

    let provider = provider.read().await;
    let album_id = track.album_id.to_string();

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

    if !provider.has_album(&album_id).await {
        return (StatusCode::NOT_FOUND, [(CACHE_CONTROL, "private")]).into_response();
    }

    let transcoder = query.get_transcoder(claim.is_guest());
    let need_range = need_range && !transcoder.need_transcode(); // Only support range if transcode is not performed

    // range is only supported on lossless
    #[cfg(feature = "transcode")]
    let range = if transcoder.need_transcode() {
        Range::FULL
    } else {
        range
    };

    let audio = provider
        .get_audio(&album_id, track.disc_id, track.track_id, range)
        .await
        .map_err(|_| AnnilError::NotFound);

    return match audio {
        Ok(audio) => {
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
                "X-Origin-Type, X-Origin-Size, X-Duration-Seconds, X-Audio-Quality".to_string(),
            )];

            let headers = [
                ("X-Origin-Type", format!("audio/{}", audio.info.extension)),
                ("X-Origin-Size", format!("{}", audio.info.size)),
                (
                    "X-Duration-Seconds",
                    format!("{}", audio.info.duration / 1000),
                ),
                (
                    "X-Audio-Quality",
                    query.quality(claim.is_guest()).as_str().to_string(),
                ),
            ];

            #[cfg(feature = "transcode")]
            let body = if transcoder.quality().need_transcode() {
                let mut transcode_headers = HeaderMap::new();
                let info = audio.info.clone();
                let mut process = transcoder.spawn();
                let stdout = process.stdout.take().unwrap();
                tokio::spawn(async move {
                    let mut stdin = process.stdin.as_mut().unwrap();
                    let mut audio = audio;
                    let _ = tokio::io::copy(&mut audio.reader, &mut stdin).await;
                });
                transcode_headers.insert(
                    CONTENT_TYPE,
                    transcoder.content_type().to_string().parse().unwrap(),
                );
                if let Some(length) = transcoder.content_length(&info) {
                    transcode_headers.insert(CONTENT_LENGTH, length.to_string().parse().unwrap());
                }

                if let Some(length) = transcoder.content_length(&info) {
                    Either::Left((
                        transcode_headers,
                        Either::Left(Body::from_stream(ReaderStream::new(stdout).take(length))),
                    ))
                } else {
                    Either::Left((
                        transcode_headers,
                        Either::Right(Body::from_stream(ReaderStream::new(stdout))),
                    ))
                }
            } else {
                let size = audio.range.length().unwrap_or(audio.info.size as u64);
                Either::Right((
                    [
                        (CONTENT_TYPE, format!("audio/{}", audio.info.extension)),
                        (CONTENT_LENGTH, format!("{size}")),
                    ],
                    Body::from_stream(ReaderStream::new(audio.reader).take(size as usize)),
                ))
            };

            #[cfg(not(feature = "transcode"))]
            let body = {
                let size = audio.range.length().unwrap_or(audio.info.size as u64);
                (
                    [
                        (CONTENT_LENGTH, format!("{size}")),
                        (CONTENT_TYPE, format!("audio/{}", audio.info.extension)),
                    ],
                    Body::from_stream(ReaderStream::new(audio.reader).take(size as usize)),
                )
            };

            (status, range, header, headers, body).into_response()
        }
        Err(e) => e.into_response(),
    };
}
