use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SplitError {
    #[error(transparent)]
    ExecutableNotFound(#[from] which::Error),

    #[error(transparent)]
    IOError(#[from] io::Error),
}
