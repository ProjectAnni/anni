//! Domain rules shared by the Anni ingest backend and workers.
//!
//! This crate deliberately contains no database, HTTP, AI, or filesystem code.
//! Keeping the workflow state machine free of side effects makes its safety
//! invariants reviewable and cheap to test.

mod digest;
mod job;
mod manifest;
mod metadata;
mod path;
mod plan;

pub use digest::{Digest, ParseDigestError};
pub use job::{
    IngestJob, IngestJobSnapshot, JobError, JobOperation, JobState, MetadataRevision,
    PlanReference, SnapshotError, UnknownJobState, VerificationReference,
};
pub use manifest::{AudioFormat, InputFileKind, InputManifest, ManifestEntry, ManifestError};
pub use metadata::{
    AlbumField, AlbumLayout, CompletenessReport, Confidence, DiscField, Evidence, EvidenceMethod,
    EvidenceSourceKind, FieldPath, MetadataCandidate, MetadataDecision, MetadataDraft,
    MetadataDraftSnapshot, MetadataError, MetadataProfile, MetadataRequirements,
    MetadataReviewContext, MetadataValue, MetadataValueKind, TrackField,
};
pub use path::{PathError, SafeRelativePath};
pub use plan::{ExecutionPlan, PlanError, PlanOperation, SplitOutputFormat};
