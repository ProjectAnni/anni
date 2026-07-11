use anni_ingest::{Digest, JobError, JobState, MetadataError, MetadataRevision};
use async_graphql::{
    Context, Enum, Error, ErrorExtensions, InputObject, OneofObject, Result, SimpleObject,
};
use chrono::{DateTime, Utc};
use sea_orm::prelude::Uuid;
use tokio_stream::{
    wrappers::{errors::BroadcastStreamRecvError, BroadcastStream},
    Stream, StreamExt,
};

use crate::ingest::{
    IngestCommand, IngestRepositoryError, IngestService, IngestServiceError, RowVersion,
    VersionedIngestJob,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum IngestJobState {
    Created,
    Reviewing,
    Planned,
    Executing,
    Verifying,
    ReadyToCommit,
    Committing,
    Published,
    Quarantined,
    Cancelled,
}

impl From<JobState> for IngestJobState {
    fn from(state: JobState) -> Self {
        match state {
            JobState::Created => Self::Created,
            JobState::Reviewing => Self::Reviewing,
            JobState::Planned => Self::Planned,
            JobState::Executing => Self::Executing,
            JobState::Verifying => Self::Verifying,
            JobState::ReadyToCommit => Self::ReadyToCommit,
            JobState::Committing => Self::Committing,
            JobState::Published => Self::Published,
            JobState::Quarantined => Self::Quarantined,
            JobState::Cancelled => Self::Cancelled,
        }
    }
}

impl From<IngestJobState> for JobState {
    fn from(state: IngestJobState) -> Self {
        match state {
            IngestJobState::Created => Self::Created,
            IngestJobState::Reviewing => Self::Reviewing,
            IngestJobState::Planned => Self::Planned,
            IngestJobState::Executing => Self::Executing,
            IngestJobState::Verifying => Self::Verifying,
            IngestJobState::ReadyToCommit => Self::ReadyToCommit,
            IngestJobState::Committing => Self::Committing,
            IngestJobState::Published => Self::Published,
            IngestJobState::Quarantined => Self::Quarantined,
            IngestJobState::Cancelled => Self::Cancelled,
        }
    }
}

/// String-encoded counters avoid JavaScript's integer precision limit.
#[derive(Debug, Clone, PartialEq, Eq, SimpleObject)]
pub struct IngestJobInfo {
    job_id: Uuid,
    state: IngestJobState,
    metadata_revision: String,
    approved_revision: Option<String>,
    manifest_digest: Option<String>,
    plan_digest: Option<String>,
    verification_digest: Option<String>,
    row_version: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<VersionedIngestJob> for IngestJobInfo {
    fn from(versioned: VersionedIngestJob) -> Self {
        let snapshot = versioned.job().snapshot();
        Self {
            job_id: snapshot.id(),
            state: snapshot.state().into(),
            metadata_revision: snapshot.metadata_revision().to_string(),
            approved_revision: snapshot
                .approved_revision()
                .map(|revision| revision.to_string()),
            manifest_digest: snapshot.manifest_digest().map(|digest| digest.to_string()),
            plan_digest: snapshot.plan_digest().map(|digest| digest.to_string()),
            verification_digest: snapshot
                .verification_digest()
                .map(|digest| digest.to_string()),
            row_version: versioned.row_version().to_string(),
            created_at: versioned.created_at(),
            updated_at: versioned.updated_at(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum IngestCommandSignal {
    Execute,
}

#[derive(Debug, InputObject)]
pub struct IngestRevisionCommandInput {
    expected_revision: String,
}

#[derive(Debug, InputObject)]
pub struct IngestCreatePlanCommandInput {
    expected_revision: String,
    manifest_digest: String,
    plan_digest: String,
}

#[derive(Debug, InputObject)]
pub struct IngestPlanCommandInput {
    plan_digest: String,
}

#[derive(Debug, InputObject)]
pub struct IngestVerificationCommandInput {
    plan_digest: String,
    verification_digest: String,
}

#[derive(Debug, OneofObject)]
pub enum IngestJobCommandInput {
    BeginReview(IngestCommandSignal),
    ApproveRevision(IngestRevisionCommandInput),
    ReviseMetadata(IngestRevisionCommandInput),
    CreatePlan(IngestCreatePlanCommandInput),
    BeginExecution(IngestPlanCommandInput),
    BeginVerification(IngestPlanCommandInput),
    AcceptVerification(IngestVerificationCommandInput),
    BeginCommit(IngestVerificationCommandInput),
    Publish(IngestVerificationCommandInput),
    Quarantine(IngestCommandSignal),
    Cancel(IngestCommandSignal),
}

#[derive(Debug, InputObject)]
pub struct ExecuteIngestJobCommandInput {
    job_id: Uuid,
    expected_row_version: String,
    command: IngestJobCommandInput,
}

impl TryFrom<IngestJobCommandInput> for IngestCommand {
    type Error = Error;

    fn try_from(input: IngestJobCommandInput) -> Result<Self> {
        match input {
            IngestJobCommandInput::BeginReview(_) => Ok(Self::BeginReview),
            IngestJobCommandInput::ApproveRevision(input) => Ok(Self::ApproveRevision {
                expected_revision: parse_revision(&input.expected_revision)?,
            }),
            IngestJobCommandInput::ReviseMetadata(input) => Ok(Self::ReviseMetadata {
                expected_revision: parse_revision(&input.expected_revision)?,
            }),
            IngestJobCommandInput::CreatePlan(input) => Ok(Self::CreatePlan {
                expected_revision: parse_revision(&input.expected_revision)?,
                manifest_digest: parse_digest(&input.manifest_digest)?,
                plan_digest: parse_digest(&input.plan_digest)?,
            }),
            IngestJobCommandInput::BeginExecution(input) => Ok(Self::BeginExecution {
                plan_digest: parse_digest(&input.plan_digest)?,
            }),
            IngestJobCommandInput::BeginVerification(input) => Ok(Self::BeginVerification {
                plan_digest: parse_digest(&input.plan_digest)?,
            }),
            IngestJobCommandInput::AcceptVerification(input) => Ok(Self::AcceptVerification {
                plan_digest: parse_digest(&input.plan_digest)?,
                verification_digest: parse_digest(&input.verification_digest)?,
            }),
            IngestJobCommandInput::BeginCommit(input) => Ok(Self::BeginCommit {
                plan_digest: parse_digest(&input.plan_digest)?,
                verification_digest: parse_digest(&input.verification_digest)?,
            }),
            IngestJobCommandInput::Publish(input) => Ok(Self::Publish {
                plan_digest: parse_digest(&input.plan_digest)?,
                verification_digest: parse_digest(&input.verification_digest)?,
            }),
            IngestJobCommandInput::Quarantine(_) => Ok(Self::Quarantine),
            IngestJobCommandInput::Cancel(_) => Ok(Self::Cancel),
        }
    }
}

pub async fn query_job(ctx: &Context<'_>, job_id: Uuid) -> Result<Option<IngestJobInfo>> {
    let service = ctx.data::<IngestService>()?;
    service
        .get(job_id)
        .await
        .map(|job| job.map(Into::into))
        .map_err(service_error)
}

pub async fn query_jobs(
    ctx: &Context<'_>,
    state: Option<IngestJobState>,
    limit: i32,
    offset: i32,
) -> Result<Vec<IngestJobInfo>> {
    if !(1..=200).contains(&limit) || offset < 0 {
        return Err(input_error(
            "INGEST_INVALID_PAGINATION",
            "limit must be between 1 and 200 and offset must be non-negative",
        ));
    }

    let service = ctx.data::<IngestService>()?;
    service
        .list(
            state.map(Into::into),
            u64::try_from(limit).expect("positive i32 fits u64"),
            u64::try_from(offset).expect("non-negative i32 fits u64"),
        )
        .await
        .map(|jobs| jobs.into_iter().map(Into::into).collect())
        .map_err(service_error)
}

pub async fn create_job(ctx: &Context<'_>, job_id: Option<Uuid>) -> Result<IngestJobInfo> {
    let service = ctx.data::<IngestService>()?;
    service
        .create(job_id)
        .await
        .map(Into::into)
        .map_err(service_error)
}

pub async fn execute_command(
    ctx: &Context<'_>,
    input: ExecuteIngestJobCommandInput,
) -> Result<IngestJobInfo> {
    let row_version = parse_row_version(&input.expected_row_version)?;
    let command = input.command.try_into()?;
    let service = ctx.data::<IngestService>()?;
    service
        .execute(input.job_id, row_version, command)
        .await
        .map(Into::into)
        .map_err(service_error)
}

pub fn subscribe_jobs(
    ctx: &Context<'_>,
    job_id: Option<Uuid>,
    after_row_version: Option<String>,
) -> Result<impl Stream<Item = Result<IngestJobInfo>> + use<>> {
    let after_row_version = after_row_version
        .as_deref()
        .map(parse_row_version)
        .transpose()?;
    let receiver = ctx.data::<IngestService>()?.subscribe();

    Ok(
        BroadcastStream::new(receiver).filter_map(move |event| match event {
            Ok(event) => {
                let versioned = event.job();
                let matches_job = job_id.is_none_or(|job_id| versioned.job().id() == job_id);
                let is_newer = after_row_version
                    .is_none_or(|row_version| versioned.row_version() > row_version);
                (matches_job && is_newer).then(|| Ok(versioned.clone().into()))
            }
            Err(BroadcastStreamRecvError::Lagged(skipped)) => Some(Err(Error::new(format!(
            "ingest event stream fell behind by {skipped} updates; query the current job snapshot"
        ))
            .extend_with(|_, extensions| {
                extensions.set("code", "INGEST_EVENT_LAGGED");
                extensions.set("skipped", skipped.to_string());
            }))),
        }),
    )
}

pub(super) fn parse_revision(value: &str) -> Result<MetadataRevision> {
    let revision = value.parse::<u64>().map_err(|_| {
        input_error(
            "INGEST_INVALID_REVISION",
            "metadata revision must be a positive base-10 integer",
        )
    })?;
    MetadataRevision::new(revision).ok_or_else(|| {
        input_error(
            "INGEST_INVALID_REVISION",
            "metadata revision must be greater than zero",
        )
    })
}

pub(super) fn parse_row_version(value: &str) -> Result<RowVersion> {
    let version = value.parse::<u64>().map_err(|_| {
        input_error(
            "INGEST_INVALID_ROW_VERSION",
            "row version must be a positive base-10 integer",
        )
    })?;
    RowVersion::new(version).ok_or_else(|| {
        input_error(
            "INGEST_INVALID_ROW_VERSION",
            "row version must be greater than zero",
        )
    })
}

fn parse_digest(value: &str) -> Result<Digest> {
    value.parse().map_err(|error| {
        input_error(
            "INGEST_INVALID_DIGEST",
            format!("invalid content digest: {error}"),
        )
    })
}

pub(super) fn input_error(code: &'static str, message: impl Into<String>) -> Error {
    Error::new(message).extend_with(|_, extensions| extensions.set("code", code))
}

pub(super) fn service_error(error: IngestServiceError) -> Error {
    match error {
        IngestServiceError::Repository(IngestRepositoryError::AlreadyExists { job_id }) => {
            Error::new(format!("ingest job {job_id} already exists")).extend_with(
                |_, extensions| {
                    extensions.set("code", "INGEST_JOB_ALREADY_EXISTS");
                    extensions.set("jobId", job_id.to_string());
                },
            )
        }
        IngestServiceError::Repository(IngestRepositoryError::NotFound { job_id }) => {
            Error::new(format!("ingest job {job_id} was not found")).extend_with(|_, extensions| {
                extensions.set("code", "INGEST_JOB_NOT_FOUND");
                extensions.set("jobId", job_id.to_string());
            })
        }
        IngestServiceError::Repository(IngestRepositoryError::ConcurrentModification {
            job_id,
            expected,
            actual,
        }) => Error::new(format!("ingest job {job_id} changed concurrently")).extend_with(
            |_, extensions| {
                extensions.set("code", "INGEST_JOB_CONFLICT");
                extensions.set("jobId", job_id.to_string());
                extensions.set("expectedRowVersion", expected.to_string());
                extensions.set("actualRowVersion", actual.to_string());
            },
        ),
        IngestServiceError::Repository(IngestRepositoryError::MetadataFrozen {
            job_id,
            revision,
        }) => Error::new(format!(
            "metadata revision {revision} for ingest job {job_id} is frozen"
        ))
        .extend_with(|_, extensions| {
            extensions.set("code", "INGEST_METADATA_FROZEN");
            extensions.set("jobId", job_id.to_string());
            extensions.set("revision", revision.to_string());
        }),
        IngestServiceError::Repository(IngestRepositoryError::MetadataNotFound {
            job_id,
            revision,
        }) => Error::new(format!(
            "metadata revision {revision} for ingest job {job_id} was not found"
        ))
        .extend_with(|_, extensions| {
            extensions.set("code", "INGEST_METADATA_NOT_FOUND");
            extensions.set("jobId", job_id.to_string());
            extensions.set("revision", revision.to_string());
        }),
        IngestServiceError::Repository(
            error @ (IngestRepositoryError::InvalidMetadataDigestLength { .. }
            | IngestRepositoryError::MetadataDigestMismatch { .. }
            | IngestRepositoryError::InvalidMetadataDocument { .. }
            | IngestRepositoryError::InvalidPersistedMetadata { .. }
            | IngestRepositoryError::MetadataDocumentRevisionMismatch { .. }),
        ) => {
            tracing::error!(error = ?error, "persisted ingest metadata is corrupt");
            Error::new("persisted ingest metadata is corrupt")
                .extend_with(|_, extensions| extensions.set("code", "INGEST_METADATA_CORRUPT"))
        }
        IngestServiceError::Domain(JobError::InvalidTransition { state, operation }) => Error::new(
            format!("cannot {operation} while ingest job is {state}"),
        )
        .extend_with(|_, extensions| {
            extensions.set("code", "INGEST_INVALID_TRANSITION");
            extensions.set("state", state.as_str());
            extensions.set("operation", operation.to_string());
        }),
        IngestServiceError::Domain(JobError::RevisionConflict { expected, actual }) => Error::new(
            format!("metadata revision conflict: expected {expected}, actual {actual}"),
        )
        .extend_with(|_, extensions| {
            extensions.set("code", "INGEST_METADATA_REVISION_CONFLICT");
            extensions.set("expectedRevision", expected.to_string());
            extensions.set("actualRevision", actual.to_string());
        }),
        IngestServiceError::Metadata(MetadataError::UnknownCandidate { id }) => Error::new(
            format!("metadata candidate {id} was not found"),
        )
        .extend_with(|_, extensions| {
            extensions.set("code", "INGEST_METADATA_CANDIDATE_NOT_FOUND");
            extensions.set("candidateId", id.to_string());
        }),
        IngestServiceError::Metadata(MetadataError::FieldOutsideAlbumLayout { field }) => {
            Error::new(format!(
                "metadata field {field:?} falls outside the configured album layout"
            ))
            .extend_with(|_, extensions| {
                extensions.set("code", "INGEST_METADATA_INVALID_FIELD");
                extensions.set("field", format!("{field:?}"));
            })
        }
        IngestServiceError::Metadata(error @ MetadataError::ValueKindMismatch { .. }) => {
            Error::new(error.to_string()).extend_with(|_, extensions| {
                extensions.set("code", "INGEST_METADATA_INVALID_VALUE")
            })
        }
        IngestServiceError::Metadata(error @ MetadataError::EvidenceMethodMismatch { .. }) => {
            Error::new(error.to_string()).extend_with(|_, extensions| {
                extensions.set("code", "INGEST_METADATA_INVALID_EVIDENCE")
            })
        }
        IngestServiceError::Metadata(MetadataError::ReviewContextMissing) => Error::new(
            "metadata review context has not been configured",
        )
        .extend_with(|_, extensions| extensions.set("code", "INGEST_METADATA_CONTEXT_REQUIRED")),
        IngestServiceError::Metadata(error) => Error::new(error.to_string())
            .extend_with(|_, extensions| extensions.set("code", "INGEST_METADATA_INVALID")),
        IngestServiceError::MetadataIncomplete { missing } => {
            let missing_fields = missing
                .iter()
                .map(|field| format!("{field:?}"))
                .collect::<Vec<_>>()
                .join(",");
            Error::new(format!(
                "metadata revision is incomplete: {} required fields are missing",
                missing.len()
            ))
            .extend_with(|_, extensions| {
                extensions.set("code", "INGEST_METADATA_INCOMPLETE");
                extensions.set("missingCount", missing.len().to_string());
                extensions.set("missingFields", missing_fields);
            })
        }
        IngestServiceError::Domain(error) => Error::new(error.to_string())
            .extend_with(|_, extensions| extensions.set("code", "INGEST_INVALID_COMMAND")),
        internal => {
            tracing::error!(error = ?internal, "ingest service request failed");
            Error::new("internal ingest service error")
                .extend_with(|_, extensions| extensions.set("code", "INTERNAL"))
        }
    }
}
