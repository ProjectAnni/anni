use crate::extractor::token::AnnilClaim;
use crate::AppState;
use axum::extract::State;
use axum::http::header::{ETAG, IF_NONE_MATCH};
use axum::http::{Request, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::Arc;

/// Get available albums of current annil server
pub async fn albums<B>(
    claims: AnnilClaim,
    State(data): State<Arc<AppState>>,
    req: Request<B>,
) -> Response {
    match claims {
        AnnilClaim::User(_) => {
            if let Some(Ok(mut etag)) = req.headers().get(IF_NONE_MATCH).map(|v| v.to_str()) {
                if let Some(etag_now) = data.etag.read().await.as_deref() {
                    if etag.starts_with("W/") {
                        etag = &etag[2..];
                    }
                    if etag == etag_now {
                        return StatusCode::NOT_MODIFIED.into_response();
                    }
                }
            }

            let mut albums: HashSet<Cow<str>> = HashSet::new();
            let read = data.providers.read().await;

            // users can get real album list
            for provider in read.iter() {
                albums.extend(provider.albums().await);
            }

            if let Some(etag) = data.etag.read().await.as_deref() {
                (axum::response::AppendHeaders([(ETAG, etag)]), Json(albums)).into_response()
            } else {
                Json(albums).into_response()
            }
        }
        AnnilClaim::Share(share) => {
            // guests can only get album list defined in jwt
            Json(share.audios.keys().collect::<Vec<_>>()).into_response()
        }
    }
}
