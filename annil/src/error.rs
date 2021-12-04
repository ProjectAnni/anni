use thiserror::Error;
use actix_web::ResponseError;
use actix_web::http::StatusCode;

#[derive(Error, Debug)]
pub enum AnnilError {
    #[error("unauthorized")]
    Unauthorized,
    #[error("no permission")]
    NoPermission,
    #[error("not found")]
    NotFound,
}

impl ResponseError for AnnilError {
    fn status_code(&self) -> StatusCode {
        match self {
            AnnilError::Unauthorized => StatusCode::UNAUTHORIZED,
            AnnilError::NoPermission => StatusCode::FORBIDDEN,
            AnnilError::NotFound => StatusCode::NOT_FOUND,
        }
    }
}
