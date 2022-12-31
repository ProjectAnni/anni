use axum::body::StreamBody;
use axum::extract::Path;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::StatusCode;
use axum::response::{AppendHeaders, IntoResponse, Response};
use axum::Extension;
use std::num::NonZeroU8;
use std::sync::Arc;

use crate::AppState;
use serde::Deserialize;
use tokio_util::io::ReaderStream;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct CoverPath {
    album_id: Uuid,
    disc_id: Option<NonZeroU8>,
}

/// Get audio cover of an album with {album_id} and optional {disc_id}
pub async fn cover(
    Path(CoverPath { album_id, disc_id }): Path<CoverPath>,
    Extension(data): Extension<Arc<AppState>>,
) -> Response {
    for provider in data.providers.read().await.iter() {
        if provider.has_album(&album_id).await {
            return match provider.get_cover(&album_id.to_string(), disc_id).await {
                Ok(cover) => (
                    ([
                        (CONTENT_TYPE, "image/jpeg"),
                        (CACHE_CONTROL, "public, max-age=31536000"),
                    ]),
                    StreamBody::new(ReaderStream::new(cover)),
                )
                    .into_response(),
                Err(_) => (
                    StatusCode::NOT_FOUND,
                    AppendHeaders([(CACHE_CONTROL, "private")]),
                )
                    .into_response(),
            };
        }
    }

    (
        StatusCode::NOT_FOUND,
        AppendHeaders([(CACHE_CONTROL, "private")]),
    )
        .into_response()
}