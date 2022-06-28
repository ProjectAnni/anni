use actix_web::http::StatusCode;
use actix_web::ResponseError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AnnilError {
    #[error("unauthorized")]
    Unauthorized,
    #[error("not found")]
    NotFound,
}

impl ResponseError for AnnilError {
    fn status_code(&self) -> StatusCode {
        match self {
            AnnilError::Unauthorized => StatusCode::FORBIDDEN,
            AnnilError::NotFound => StatusCode::NOT_FOUND,
        }
    }
}
