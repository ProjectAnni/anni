use thiserror::Error;
use std::string::FromUtf8Error;

#[derive(Error, Debug)]
pub enum FlacError {
    #[error("invalid magic number")]
    InvalidMagicNumber,
    #[error("invalid block type 0xff")]
    InvalidBlockType,
    #[error("invalid seektable size")]
    InvalidSeekTableSize,
    #[error(transparent)]
    InvalidString(#[from] FromUtf8Error),
    #[error(transparent)]
    IO(#[from] std::io::Error),
}