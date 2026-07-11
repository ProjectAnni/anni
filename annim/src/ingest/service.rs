use anni_ingest::{Digest, IngestJob, JobError, JobState, MetadataRevision};
use sea_orm::prelude::Uuid;
use thiserror::Error;
use tokio::sync::broadcast;

use super::{IngestJobRepository, IngestRepositoryError, RowVersion, VersionedIngestJob};

const EVENT_CHANNEL_CAPACITY: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestJobEvent {
    job: VersionedIngestJob,
}

impl IngestJobEvent {
    pub const fn job(&self) -> &VersionedIngestJob {
        &self.job
    }
}

#[derive(Debug, Clone)]
struct IngestEventHub {
    sender: broadcast::Sender<IngestJobEvent>,
}

impl Default for IngestEventHub {
    fn default() -> Self {
        let (sender, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self { sender }
    }
}

impl IngestEventHub {
    fn publish(&self, job: &VersionedIngestJob) {
        // Having no live Web clients is normal and must not fail a committed
        // database write.
        let _ = self.sender.send(IngestJobEvent { job: job.clone() });
    }

    fn subscribe(&self) -> broadcast::Receiver<IngestJobEvent> {
        self.sender.subscribe()
    }
}

/// Complete command vocabulary shared by Web requests and background workers.
/// Values are parsed into domain types before this boundary is crossed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IngestCommand {
    BeginReview,
    ApproveRevision {
        expected_revision: MetadataRevision,
    },
    ReviseMetadata {
        expected_revision: MetadataRevision,
    },
    CreatePlan {
        expected_revision: MetadataRevision,
        manifest_digest: Digest,
        plan_digest: Digest,
    },
    BeginExecution {
        plan_digest: Digest,
    },
    BeginVerification {
        plan_digest: Digest,
    },
    AcceptVerification {
        plan_digest: Digest,
        verification_digest: Digest,
    },
    BeginCommit {
        plan_digest: Digest,
        verification_digest: Digest,
    },
    Publish {
        plan_digest: Digest,
        verification_digest: Digest,
    },
    Quarantine,
    Cancel,
}

impl IngestCommand {
    fn apply(self, job: &mut IngestJob) -> Result<(), JobError> {
        match self {
            Self::BeginReview => job.begin_review(),
            Self::ApproveRevision { expected_revision } => job.approve_revision(expected_revision),
            Self::ReviseMetadata { expected_revision } => {
                job.revise_metadata(expected_revision).map(|_| ())
            }
            Self::CreatePlan {
                expected_revision,
                manifest_digest,
                plan_digest,
            } => job
                .create_plan(expected_revision, manifest_digest, plan_digest)
                .map(|_| ()),
            Self::BeginExecution { plan_digest } => job.begin_execution(plan_digest),
            Self::BeginVerification { plan_digest } => job.begin_verification(plan_digest),
            Self::AcceptVerification {
                plan_digest,
                verification_digest,
            } => job
                .accept_verification(plan_digest, verification_digest)
                .map(|_| ()),
            Self::BeginCommit {
                plan_digest,
                verification_digest,
            } => job.begin_commit(plan_digest, verification_digest),
            Self::Publish {
                plan_digest,
                verification_digest,
            } => job.publish(plan_digest, verification_digest),
            Self::Quarantine => job.quarantine(),
            Self::Cancel => job.cancel(),
        }
    }
}

#[derive(Debug, Error)]
pub enum IngestServiceError {
    #[error(transparent)]
    Repository(#[from] IngestRepositoryError),
    #[error(transparent)]
    Domain(#[from] JobError),
}

#[derive(Clone)]
pub struct IngestService {
    repository: IngestJobRepository,
    events: IngestEventHub,
}

impl IngestService {
    pub fn new(repository: IngestJobRepository) -> Self {
        Self {
            repository,
            events: IngestEventHub::default(),
        }
    }

    pub async fn create(
        &self,
        job_id: Option<Uuid>,
    ) -> Result<VersionedIngestJob, IngestServiceError> {
        let job = IngestJob::new(job_id.unwrap_or_else(Uuid::new_v4));
        let versioned = self.repository.create(&job).await?;
        self.events.publish(&versioned);
        Ok(versioned)
    }

    pub async fn get(
        &self,
        job_id: Uuid,
    ) -> Result<Option<VersionedIngestJob>, IngestServiceError> {
        Ok(self.repository.get(job_id).await?)
    }

    pub async fn list(
        &self,
        state: Option<JobState>,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<VersionedIngestJob>, IngestServiceError> {
        Ok(self.repository.list(state, limit, offset).await?)
    }

    pub async fn execute(
        &self,
        job_id: Uuid,
        expected_row_version: RowVersion,
        command: IngestCommand,
    ) -> Result<VersionedIngestJob, IngestServiceError> {
        let mut versioned = self
            .repository
            .get(job_id)
            .await?
            .ok_or(IngestRepositoryError::NotFound { job_id })?;

        if versioned.row_version() != expected_row_version {
            return Err(IngestRepositoryError::ConcurrentModification {
                job_id,
                expected: expected_row_version,
                actual: versioned.row_version(),
            }
            .into());
        }

        command.apply(versioned.job_mut())?;
        self.repository.save(&mut versioned).await?;
        self.events.publish(&versioned);
        Ok(versioned)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<IngestJobEvent> {
        self.events.subscribe()
    }
}
