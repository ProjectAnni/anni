use std::process::Stdio;
use actix_web::{HttpRequest, HttpResponse, Responder, ResponseError, web};
use actix_web::web::Query;
use tokio_util::io::ReaderStream;
use serde::Deserialize;
use anni_provider::Range;
use crate::{AnnilClaims, AnnilError, AppState};

#[derive(Deserialize)]
pub struct AudioQuery {
    prefer_bitrate: Option<String>,
}

pub async fn audio_head(claim: AnnilClaims, path: web::Path<(String, u8, u8)>, data: web::Data<AppState>, query: Query<AudioQuery>) -> impl Responder {
    let (album_id, disc_id, track_id) = path.into_inner();
    if !claim.can_fetch(&album_id, disc_id, track_id) {
        return AnnilError::Unauthorized.error_response();
    }

    for provider in data.providers.read().await.iter() {
        if provider.has_album(&album_id).await {
            let audio = provider.get_audio_info(&album_id, disc_id, track_id).await.map_err(|_| AnnilError::NotFound);
            return match audio {
                Ok(info) => {
                    let transcode = if claim.is_guest() { true } else { query.prefer_bitrate.as_deref().unwrap_or("medium") != "lossless" };

                    let mut resp = HttpResponse::Ok();
                    resp.append_header(("X-Origin-Type", format!("audio/{}", info.extension)))
                        .append_header(("X-Origin-Size", info.size))
                        .append_header(("X-Duration-Seconds", info.duration))
                        .append_header(("Access-Control-Expose-Headers", "X-Origin-Type, X-Origin-Size, X-Duration-Seconds"))
                        .content_type(if transcode {
                            "audio/aac".to_string()
                        } else {
                            format!("audio/{}", info.extension)
                        });
                    resp.finish()
                }
                Err(e) => {
                    e.error_response()
                }
            };
        }
    }
    HttpResponse::NotFound().finish()
}

/// Get audio in an album with {album_id}, {disc_id} and {track_id}
pub async fn audio(claim: AnnilClaims, path: web::Path<(String, u8, u8)>, data: web::Data<AppState>, query: Query<AudioQuery>, req: HttpRequest) -> impl Responder {
    let (album_id, disc_id, track_id) = path.into_inner();
    if !claim.can_fetch(&album_id, disc_id, track_id) {
        return AnnilError::Unauthorized.error_response();
    }

    let prefer_bitrate = if claim.is_guest() { "low" } else { query.prefer_bitrate.as_deref().unwrap_or("medium") };
    let bitrate = match prefer_bitrate {
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
        Some(Range::new(from.parse().ok()?, to.parse().ok()))
    }).unwrap_or(Range::FULL);

    for provider in data.providers.read().await.iter() {
        if provider.has_album(&album_id).await {
            // range is only supported on lossless
            let range = if bitrate.is_some() { Range::FULL } else { range };
            let audio = provider.get_audio(&album_id, disc_id, track_id, range).await.map_err(|_| AnnilError::NotFound);
            if let Err(e) = audio {
                return e.error_response();
            }

            let mut audio = audio.unwrap();
            let mut resp = if !audio.range.is_full() {
                let mut resp = HttpResponse::PartialContent();
                resp.append_header(("Content-Range", audio.range.to_content_range_header()));
                resp
            } else { HttpResponse::Ok() };

            resp.append_header(("X-Origin-Type", format!("audio/{}", audio.info.extension)))
                .append_header(("X-Origin-Size", audio.info.size))
                .append_header(("X-Duration-Seconds", audio.info.duration))
                .append_header(("Access-Control-Expose-Headers", "X-Origin-Type, X-Origin-Size, X-Duration-Seconds"))
                .content_type(match bitrate {
                    Some(_) => "audio/aac".to_string(),
                    None => format!("audio/{}", audio.info.extension)
                });

            return match bitrate {
                Some(bitrate) => {
                    let mut process = tokio::process::Command::new("ffmpeg")
                        .args(&[
                            "-i", "pipe:0",
                            "-map", "0:0",
                            "-b:a", bitrate,
                            "-f", "adts",
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
                    resp.streaming(ReaderStream::new(stdout))
                }
                None => {
                    resp.streaming(ReaderStream::new(audio.reader))
                }
            };
        }
    }
    HttpResponse::NotFound().finish()
}