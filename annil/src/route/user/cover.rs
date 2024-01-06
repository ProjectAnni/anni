use axum::body::Body;
use axum::extract::Path;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Extension;
use std::num::NonZeroU8;
use std::sync::Arc;

use crate::provider::AnnilProvider;
use anni_provider::AnniProvider;
use serde::Deserialize;
use tokio_util::io::ReaderStream;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct CoverPath {
    album_id: Uuid,
    disc_id: Option<NonZeroU8>,
}

/// Get audio cover of an album with {album_id} and optional {disc_id}
pub async fn cover<P>(
    Path(CoverPath { album_id, disc_id }): Path<CoverPath>,
    Extension(provider): Extension<Arc<AnnilProvider<P>>>,
) -> Response
where
    P: AnniProvider + Send + Sync,
{
    let provider = provider.read().await;
    let album_id = album_id.to_string();

    if !provider.has_album(&album_id).await {
        return (StatusCode::NOT_FOUND, [(CACHE_CONTROL, "private")]).into_response();
    }

    match provider.get_cover(&album_id, disc_id).await {
        Ok(cover) => (
            ([
                (CONTENT_TYPE, "image/jpeg"),
                (CACHE_CONTROL, "public, max-age=31536000"),
            ]),
            Body::from_stream(ReaderStream::new(cover)),
        )
            .into_response(),
        Err(_) => (StatusCode::NOT_FOUND, [(CACHE_CONTROL, "private")]).into_response(),
    }
}
