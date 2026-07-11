use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SplitError {
    #[error(transparent)]
    ExecutableNotFound(#[from] which::Error),

    #[error(transparent)]
    CueError(#[from] cuna::error::Error),

    #[error("single-input splitting requires exactly one CUE FILE block, got {actual}")]
    UnsupportedCueFileCount { actual: usize },

    #[error(
        "codec command {command:?} failed with status {status:?}{stderr_suffix}",
        stderr_suffix = if stderr.is_empty() { String::new() } else { format!(": {stderr}") }
    )]
    CommandFailed {
        command: String,
        status: Option<i32>,
        stderr: String,
    },

    #[error(transparent)]
    DecodeError(#[from] anni_common::decode::DecodeError),

    #[error(transparent)]
    IOError(#[from] io::Error),
}
