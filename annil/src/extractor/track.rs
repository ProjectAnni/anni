use crate::error::AnnilError;
use async_trait::async_trait;
use axum::extract::{FromRequestParts, Path};
use axum::http::request::Parts;
use serde::Deserialize;
use std::num::NonZeroU8;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct TrackIdentifier {
    pub album_id: Uuid,
    pub disc_id: NonZeroU8,
    pub track_id: NonZeroU8,
}

#[async_trait]
impl<S> FromRequestParts<S> for TrackIdentifier
where
    S: Send + Sync,
{
    type Rejection = AnnilError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let Path(track) = Path::<TrackIdentifier>::from_request_parts(parts, &())
            .await
            .map_err(|_| AnnilError::UnknownPath)?;

        Ok(track)
    }
}
