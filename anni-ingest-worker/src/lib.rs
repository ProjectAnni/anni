//! Safe filesystem executor for immutable ingest plans.
//!
//! This crate is deliberately separate from `anni-ingest`: the latter defines
//! pure domain rules, while this crate is the only layer allowed to read source
//! files and write a job-specific staging directory.

mod executor;
mod receipt;
mod source;

pub use executor::StagingExecutor;
pub use receipt::{ExecutionReceipt, OutputReceipt};
pub use source::{SourceSpec, SourceTree};

use std::{io, path::PathBuf};

use anni_ingest::{Digest, ManifestError, SafeRelativePath};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WorkerError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Manifest(#[from] ManifestError),
    #[error(transparent)]
    WaveDecode(#[from] anni_common::decode::DecodeError),
    #[error(transparent)]
    CuePlan(#[from] anni_split::CueSplitPlanError),
    #[error(transparent)]
    Split(#[from] anni_split::error::SplitError),
    #[error("source root is not a directory: {path}")]
    SourceRootNotDirectory { path: PathBuf },
    #[error("source path {path} resolves outside configured root {root}")]
    SourceEscapesRoot {
        path: SafeRelativePath,
        root: PathBuf,
    },
    #[error("source path is not a regular file: {path}")]
    SourceNotFile { path: SafeRelativePath },
    #[error("source file {path} changed length: expected {expected}, actual {actual}")]
    SourceLengthChanged {
        path: SafeRelativePath,
        expected: u64,
        actual: u64,
    },
    #[error("source file {path} changed content: expected {expected}, actual {actual}")]
    SourceDigestChanged {
        path: SafeRelativePath,
        expected: Digest,
        actual: Digest,
    },
    #[error("execution plan manifest {plan} does not match supplied manifest {actual}")]
    ManifestMismatch { plan: Digest, actual: Digest },
    #[error("staging path already exists: {path}")]
    StagingAlreadyExists { path: PathBuf },
    #[error("staging directory cannot be created inside immutable source root: {path}")]
    StagingInsideSource { path: PathBuf },
    #[error("staging target resolves outside the job directory: {path}")]
    TargetEscapesStaging { path: SafeRelativePath },
    #[error("staging target already exists: {path}")]
    TargetAlreadyExists { path: SafeRelativePath },
    #[error("CUE describes {actual} tracks but plan declares {expected} outputs")]
    SplitOutputCountMismatch { expected: usize, actual: usize },
    #[error("CUE source file {cue_file:?} does not match planned WAV {planned_wave}")]
    CueWaveMismatch {
        cue_file: String,
        planned_wave: SafeRelativePath,
    },
    #[error("track {track:02} produced {actual} PCM bytes, expected {expected}")]
    ShortTrackRead {
        track: u8,
        expected: u64,
        actual: u64,
    },
    #[error("FLAC verification failed with status {status:?}: {stderr}")]
    FlacVerificationFailed { status: Option<i32>, stderr: String },
}

fn join_protocol_path(root: &std::path::Path, path: &SafeRelativePath) -> PathBuf {
    path.as_str()
        .split('/')
        .fold(root.to_owned(), |joined, component| joined.join(component))
}
