use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SplitError {
    #[error(transparent)]
    ExecutableNotFound(#[from] which::Error),

    #[error(transparent)]
    CueError(#[from] cuna::error::Error),

    #[error(transparent)]
    DecodeError(#[from] anni_common::decode::DecodeError),

    #[error(transparent)]
    IOError(#[from] io::Error),
}
