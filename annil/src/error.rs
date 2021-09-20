use thiserror::Error;
use actix_web::ResponseError;
use actix_web::http::StatusCode;

#[derive(Error, Debug)]
pub enum AnnilError {
    #[error("unauthorized")]
    Unauthorized,
    #[error("no permission")]
    NoPermission,
}

impl ResponseError for AnnilError {
    fn status_code(&self) -> StatusCode {
        match self {
            AnnilError::Unauthorized => StatusCode::UNAUTHORIZED,
            AnnilError::NoPermission => StatusCode::FORBIDDEN,
        }
    }
}
