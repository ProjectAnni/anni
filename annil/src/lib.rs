pub mod extractor;
pub mod provider;
pub mod route;
pub mod state;
pub mod utils;

#[cfg(feature = "metadata")]
pub mod metadata;

pub mod error {
    use axum::http::StatusCode;
    use axum::response::{IntoResponse, Response};
    use thiserror::Error;

    #[derive(Error, Debug)]
    pub enum AnnilError {
        #[error("unauthorized")]
        Unauthorized,
        #[error("not found")]
        NotFound,
    }

    impl IntoResponse for AnnilError {
        fn into_response(self) -> Response {
            match self {
                AnnilError::Unauthorized => StatusCode::FORBIDDEN,
                AnnilError::NotFound => StatusCode::NOT_FOUND,
            }
            .into_response()
        }
    }
}
