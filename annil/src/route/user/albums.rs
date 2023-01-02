use crate::extractor::token::AnnilClaim;
use crate::provider::AnnilProvider;
use crate::state::AnnilState;
use anni_provider::AnniProvider;
use axum::http::header::{ETAG, IF_NONE_MATCH};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use std::collections::HashSet;
use std::sync::Arc;

/// Get available albums of current annil server
pub async fn albums<P>(
    claims: AnnilClaim,
    Extension(provider): Extension<Arc<AnnilProvider<P>>>,
    Extension(data): Extension<Arc<AnnilState>>,
    headers: HeaderMap,
) -> Response
where
    P: AnniProvider + Send + Sync,
{
    match claims {
        AnnilClaim::User(_) => {
            let etag_now = data.etag.read().await.to_string();

            if let Some(Ok(mut etag)) = headers.get(IF_NONE_MATCH).map(|v| v.to_str()) {
                if etag.starts_with("W/") {
                    etag = &etag[2..];
                }
                if etag == etag_now {
                    return StatusCode::NOT_MODIFIED.into_response();
                }
            }

            // users can get real album list
            let provider = provider.read().await;
            let albums = provider.albums().await.unwrap_or(HashSet::new());
            ([(ETAG, etag_now)], Json(albums)).into_response()
        }
        AnnilClaim::Share(share) => {
            // guests can only get album list defined in jwt
            Json(share.audios.keys().collect::<Vec<_>>()).into_response()
        }
    }
}
