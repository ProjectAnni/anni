use crate::extractor::auth::AuthExtractor;
use crate::{error::AnnilError, state::AnnilKeys};
use async_trait::async_trait;
use axum::{extract::FromRequestParts, http::request::Parts, Extension};
use std::sync::Arc;

pub struct AnnilAdmin;

#[async_trait]
impl<S> FromRequestParts<S> for AnnilAdmin
where
    S: Send + Sync,
{
    type Rejection = AnnilError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let AuthExtractor(auth) = AuthExtractor::from_request_parts(parts, state).await?;
        let keys = Extension::<Arc<AnnilKeys>>::from_request_parts(parts, state)
            .await
            .expect("Failed to extract keys from extension. Please re-check your code first.");

        if auth != keys.admin_token {
            return Err(AnnilError::Unauthorized);
        }

        Ok(AnnilAdmin)
    }
}
