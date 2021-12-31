use actix_web::{HttpResponse, Responder, web};
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
    for backend in data.backends.read().await.iter() {
        if backend.has_album(&album_id).await {
            return match backend.get_cover(&album_id, disc_id).await {
                Ok(cover) => {
                    HttpResponse::Ok()
                        .content_type("image/jpeg")
                        .append_header(("Cache-Control", "public, max-age=31536000"))
                        .streaming(ReaderStream::new(cover))
                }
                Err(_) => {
                    HttpResponse::NotFound().finish()
                }
            };
        }
    }
    HttpResponse::NotFound().finish()
}