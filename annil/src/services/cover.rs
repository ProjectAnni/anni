use actix_web::{HttpResponse, Responder, web};
use actix_web::http::header::CACHE_CONTROL;
use tokio_util::io::ReaderStream;
use serde::Deserialize;
use crate::AppState;

#[derive(Deserialize)]
pub struct CoverPath {
    album_id: String,
    disc_id: Option<u8>,
}

/// Get audio cover of an album with {album_id} and optional {disc_id}
pub async fn cover(path: web::Path<CoverPath>, data: web::Data<AppState>) -> impl Responder {
    let CoverPath { album_id, disc_id } = path.into_inner();
    for provider in data.providers.read().iter() {
        if provider.has_album(&album_id).await {
            return match provider.get_cover(&album_id, disc_id).await {
                Ok(cover) => {
                    HttpResponse::Ok()
                        .content_type("image/jpeg")
                        .append_header((CACHE_CONTROL, "public, max-age=31536000"))
                        .streaming(ReaderStream::new(cover))
                }
                Err(_) => {
                    HttpResponse::NotFound()
                        .append_header((CACHE_CONTROL, "no-cache"))
                        .finish()
                }
            };
        }
    }
    HttpResponse::NotFound()
        .append_header((CACHE_CONTROL, "no-cache"))
        .finish()
}