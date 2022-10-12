use crate::{AnnilClaims, AnnilError, AppState};
use actix_web::http::header::{ACCEPT_RANGES, ACCESS_CONTROL_EXPOSE_HEADERS, CACHE_CONTROL};
use actix_web::web::Query;
use actix_web::{web, HttpRequest, HttpResponse, Responder, ResponseError};
use anni_provider::Range;
use serde::Deserialize;
use std::num::NonZeroU8;
use std::process::Stdio;
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
    claim: AnnilClaims,
    path: web::Path<(String, NonZeroU8, NonZeroU8)>,
    data: web::Data<AppState>,
    query: Query<AudioQuery>,
) -> impl Responder {
    let (album_id, disc_id, track_id) = path.into_inner();
    if !claim.can_fetch(&album_id, disc_id, track_id) {
        return AnnilError::Unauthorized.error_response();
    }

    for provider in data.providers.read().iter() {
        if provider.has_album(&album_id).await {
            let audio = provider
                .get_audio_info(&album_id, disc_id, track_id)
                .await
                .map_err(|_| AnnilError::NotFound);
            let transcode = query.need_transcode(claim.is_guest());
            return match audio {
                Ok(info) => {
                    let mut resp = HttpResponse::Ok();
                    resp.append_header(("X-Origin-Type", format!("audio/{}", info.extension)))
                        .append_header(("X-Origin-Size", info.size))
                        .append_header(("X-Duration-Seconds", info.duration))
                        .append_header(("X-Audio-Quality", query.quality(claim.is_guest())))
                        .append_header((ACCESS_CONTROL_EXPOSE_HEADERS, "X-Origin-Type, X-Origin-Size, X-Duration-Seconds, X-Audio-Quality, Accept-Ranges"))
                        .content_type(if transcode {
                            "audio/aac".to_string()
                        } else {
                            format!("audio/{}", info.extension)
                        });
                    if !transcode {
                        resp.append_header((ACCEPT_RANGES, "bytes"));
                    }
                    resp.finish()
                }
                Err(e) => e.error_response(),
            };
        }
    }
    HttpResponse::NotFound()
        .append_header((CACHE_CONTROL, "private"))
        .finish()
}

/// Get audio in an album with {album_id}, {disc_id} and {track_id}
pub async fn audio(
    claim: AnnilClaims,
    path: web::Path<(String, NonZeroU8, NonZeroU8)>,
    data: web::Data<AppState>,
    query: Query<AudioQuery>,
    req: HttpRequest,
) -> impl Responder {
    let (album_id, disc_id, track_id) = path.into_inner();
    if !claim.can_fetch(&album_id, disc_id, track_id) {
        return AnnilError::Unauthorized.error_response();
    }

    let bitrate = match query.quality(claim.is_guest()) {
        "low" => Some("128k"),
        "medium" => Some("192k"),
        "high" => Some("320k"),
        "lossless" => None,
        _ => Some("128k"),
    };
    let range = req.headers().get("Range").and_then(|r| {
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

    for provider in data.providers.read().iter() {
        if provider.has_album(&album_id).await {
            // range is only supported on lossless
            let range = if bitrate.is_some() {
                Range::FULL
            } else {
                range
            };
            let audio = provider
                .get_audio(&album_id, disc_id, track_id, range)
                .await
                .map_err(|_| AnnilError::NotFound);
            if let Err(e) = audio {
                return e.error_response();
            }

            let mut audio = audio.unwrap();
            let mut resp = if need_range && !audio.range.is_full() {
                let mut resp = HttpResponse::PartialContent();
                resp.append_header(("Content-Range", audio.range.to_content_range_header()))
                    .append_header(("Accept-Ranges", "bytes"));
                resp
            } else {
                HttpResponse::Ok()
            };

            resp.append_header(("X-Origin-Type", format!("audio/{}", audio.info.extension)))
                .append_header(("X-Origin-Size", audio.info.size))
                .append_header(("X-Duration-Seconds", audio.info.duration))
                .append_header(("X-Audio-Quality", query.quality(claim.is_guest())))
                .append_header((
                    ACCESS_CONTROL_EXPOSE_HEADERS,
                    "X-Origin-Type, X-Origin-Size, X-Duration-Seconds, X-Audio-Quality",
                ))
                .content_type(match bitrate {
                    Some(_) => "audio/aac".to_string(),
                    None => format!("audio/{}", audio.info.extension),
                });

            return match bitrate {
                Some(bitrate) => {
                    let mut process = tokio::process::Command::new("ffmpeg")
                        .args(&[
                            "-i", "pipe:0", "-map", "0:0", "-b:a", bitrate, "-f", "adts", "-",
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
                    resp.streaming(ReaderStream::new(stdout))
                }
                None => resp.streaming(ReaderStream::new(audio.reader)),
            };
        }
    }
    HttpResponse::NotFound()
        .append_header((CACHE_CONTROL, "private"))
        .finish()
}
