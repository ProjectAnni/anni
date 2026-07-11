//! Domain rules shared by the Anni ingest backend and workers.
//!
//! This crate deliberately contains no database, HTTP, AI, or filesystem code.
//! Keeping the workflow state machine free of side effects makes its safety
//! invariants reviewable and cheap to test.

mod digest;
mod job;

pub use digest::Digest;
pub use job::{
    IngestJob, IngestJobSnapshot, JobError, JobOperation, JobState, MetadataRevision,
    PlanReference, SnapshotError, UnknownJobState, VerificationReference,
};
