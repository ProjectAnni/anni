use crate::error::AnnilError;
use async_trait::async_trait;
use axum::extract::{FromRequestParts, Query};
use axum::http::request::Parts;
use serde::Deserialize;

/// Auth extractor for Annil.
///
/// Extracts auth from `Authorization` header or `auth` query parameter.
pub struct AuthExtractor(pub String);

#[derive(Deserialize)]
struct AuthQuery {
    auth: String,
}

#[async_trait]
impl<S> FromRequestParts<S> for AuthExtractor
where
    S: Send + Sync,
{
    type Rejection = AnnilError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let auth = if let Some(auth) = parts.headers.get("Authorization") {
            auth.to_str()
                .map_err(|_| AnnilError::Unauthorized)?
                .to_string()
        } else {
            let query = Query::<AuthQuery>::from_request_parts(parts, state)
                .await
                .map_err(|_| AnnilError::Unauthorized)?;
            query.auth.to_string()
        };

        Ok(Self(auth))
    }
}
