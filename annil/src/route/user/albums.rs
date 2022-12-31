use crate::extractor::token::AnnilClaim;
use crate::state::{AnnilProviders, AnnilState};
use axum::http::header::{ETAG, IF_NONE_MATCH};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::Arc;

/// Get available albums of current annil server
pub async fn albums(
    claims: AnnilClaim,
    Extension(providers): Extension<Arc<AnnilProviders>>,
    Extension(data): Extension<Arc<AnnilState>>,
    headers: HeaderMap,
) -> Response {
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

            let mut albums: HashSet<Cow<str>> = HashSet::new();
            let providers = providers.read().await;

            // users can get real album list
            for provider in providers.iter() {
                albums.extend(provider.albums().await);
            }

            ([(ETAG, etag_now)], Json(albums)).into_response()
        }
        AnnilClaim::Share(share) => {
            // guests can only get album list defined in jwt
            Json(share.audios.keys().collect::<Vec<_>>()).into_response()
        }
    }
}
