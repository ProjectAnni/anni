use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SplitError {
    #[error(transparent)]
    IOError(#[from] io::Error),
}
