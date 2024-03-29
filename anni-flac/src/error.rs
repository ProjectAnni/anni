use std::string::FromUtf8Error;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FlacError {
    #[error("invalid magic number")]
    InvalidMagicNumber,
    #[error("invalid first block, must be StreamInfo")]
    InvalidFirstBlock,
    #[error("invalid block type 0xff")]
    InvalidBlockType,
    #[error("invalid seek table size")]
    InvalidSeekTableSize,
    #[error("invalid picture type")]
    InvalidPictureType,
    #[error(transparent)]
    InvalidString(#[from] FromUtf8Error),
    #[error(transparent)]
    IO(#[from] std::io::Error),
    #[error(transparent)]
    ImageError(#[from] image::ImageError),
}
