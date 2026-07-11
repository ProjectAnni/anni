use std::{fmt, str::FromStr};

use thiserror::Error;
use uuid::Uuid;

use crate::Digest;

/// A monotonic revision of the metadata being reviewed for an ingest job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MetadataRevision(u64);

impl MetadataRevision {
    pub const INITIAL: Self = Self(1);

    pub const fn new(value: u64) -> Option<Self> {
        if value == 0 {
            None
        } else {
            Some(Self(value))
        }
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    fn next(self) -> Result<Self, JobError> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(JobError::RevisionExhausted)
    }
}

impl fmt::Display for MetadataRevision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// The externally visible state of an ingest job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JobState {
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

impl JobState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Reviewing => "reviewing",
            Self::Planned => "planned",
            Self::Executing => "executing",
            Self::Verifying => "verifying",
            Self::ReadyToCommit => "ready_to_commit",
            Self::Committing => "committing",
            Self::Published => "published",
            Self::Quarantined => "quarantined",
            Self::Cancelled => "cancelled",
        }
    }
}

impl fmt::Display for JobState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for JobState {
    type Err = UnknownJobState;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "created" => Ok(Self::Created),
            "reviewing" => Ok(Self::Reviewing),
            "planned" => Ok(Self::Planned),
            "executing" => Ok(Self::Executing),
            "verifying" => Ok(Self::Verifying),
            "ready_to_commit" => Ok(Self::ReadyToCommit),
            "committing" => Ok(Self::Committing),
            "published" => Ok(Self::Published),
            "quarantined" => Ok(Self::Quarantined),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(UnknownJobState(value.to_owned())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("unknown ingest job state: {0}")]
pub struct UnknownJobState(String);

/// A plan is valid only for the approved metadata revision and input manifest
/// from which it was produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanReference {
    metadata_revision: MetadataRevision,
    manifest_digest: Digest,
    plan_digest: Digest,
}

impl PlanReference {
    pub const fn metadata_revision(self) -> MetadataRevision {
        self.metadata_revision
    }

    pub const fn manifest_digest(self) -> Digest {
        self.manifest_digest
    }

    pub const fn plan_digest(self) -> Digest {
        self.plan_digest
    }
}

/// A verification receipt is bound to exactly one immutable plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerificationReference {
    plan_digest: Digest,
    receipt_digest: Digest,
}

impl VerificationReference {
    pub const fn plan_digest(self) -> Digest {
        self.plan_digest
    }

    pub const fn receipt_digest(self) -> Digest {
        self.receipt_digest
    }
}

/// Operations are typed so transition errors remain stable and machine-readable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobOperation {
    BeginReview,
    ApproveRevision,
    ReviseMetadata,
    CreatePlan,
    BeginExecution,
    BeginVerification,
    AcceptVerification,
    BeginCommit,
    Publish,
    Quarantine,
    Cancel,
}

impl fmt::Display for JobOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::BeginReview => "begin review",
            Self::ApproveRevision => "approve revision",
            Self::ReviseMetadata => "revise metadata",
            Self::CreatePlan => "create plan",
            Self::BeginExecution => "begin execution",
            Self::BeginVerification => "begin verification",
            Self::AcceptVerification => "accept verification",
            Self::BeginCommit => "begin commit",
            Self::Publish => "publish",
            Self::Quarantine => "quarantine",
            Self::Cancel => "cancel",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum JobError {
    #[error("cannot {operation} while job is {state}")]
    InvalidTransition {
        state: JobState,
        operation: JobOperation,
    },
    #[error("metadata revision conflict: expected {expected}, actual {actual}")]
    RevisionConflict {
        expected: MetadataRevision,
        actual: MetadataRevision,
    },
    #[error("metadata revision {actual} has not been approved")]
    RevisionNotApproved { actual: MetadataRevision },
    #[error("job has no immutable import plan")]
    MissingPlan,
    #[error("plan digest mismatch: expected {expected}, actual {actual}")]
    PlanMismatch { expected: Digest, actual: Digest },
    #[error("job has no accepted verification receipt")]
    MissingVerification,
    #[error("verification digest mismatch: expected {expected}, actual {actual}")]
    VerificationMismatch { expected: Digest, actual: Digest },
    #[error("metadata revision counter is exhausted")]
    RevisionExhausted,
}

/// The side-effect-free workflow aggregate used by the backend.
///
/// Only methods on this type may change workflow state. HTTP handlers and
/// workers are expected to submit commands to the backend rather than mutate
/// persisted fields independently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestJob {
    id: Uuid,
    state: JobState,
    metadata_revision: MetadataRevision,
    approved_revision: Option<MetadataRevision>,
    plan: Option<PlanReference>,
    verification: Option<VerificationReference>,
}

impl IngestJob {
    pub fn new(id: Uuid) -> Self {
        Self {
            id,
            state: JobState::Created,
            metadata_revision: MetadataRevision::INITIAL,
            approved_revision: None,
            plan: None,
            verification: None,
        }
    }

    pub const fn id(&self) -> Uuid {
        self.id
    }

    pub const fn state(&self) -> JobState {
        self.state
    }

    pub const fn metadata_revision(&self) -> MetadataRevision {
        self.metadata_revision
    }

    pub const fn approved_revision(&self) -> Option<MetadataRevision> {
        self.approved_revision
    }

    pub const fn plan(&self) -> Option<PlanReference> {
        self.plan
    }

    pub const fn verification(&self) -> Option<VerificationReference> {
        self.verification
    }

    pub fn begin_review(&mut self) -> Result<(), JobError> {
        self.require_state(JobState::Created, JobOperation::BeginReview)?;
        self.state = JobState::Reviewing;
        Ok(())
    }

    pub fn approve_revision(
        &mut self,
        expected_revision: MetadataRevision,
    ) -> Result<(), JobError> {
        self.require_state(JobState::Reviewing, JobOperation::ApproveRevision)?;
        self.require_revision(expected_revision)?;
        self.approved_revision = Some(self.metadata_revision);
        Ok(())
    }

    /// Start a new metadata revision and invalidate all derived approvals.
    ///
    /// Revisions cannot change while a worker is actively executing or while a
    /// commit is in progress. The backend must first move such work to a safe
    /// checkpoint or quarantine it.
    pub fn revise_metadata(
        &mut self,
        expected_revision: MetadataRevision,
    ) -> Result<MetadataRevision, JobError> {
        if !matches!(
            self.state,
            JobState::Reviewing
                | JobState::Planned
                | JobState::ReadyToCommit
                | JobState::Quarantined
        ) {
            return Err(self.invalid_transition(JobOperation::ReviseMetadata));
        }
        self.require_revision(expected_revision)?;

        self.metadata_revision = self.metadata_revision.next()?;
        self.approved_revision = None;
        self.plan = None;
        self.verification = None;
        self.state = JobState::Reviewing;
        Ok(self.metadata_revision)
    }

    pub fn create_plan(
        &mut self,
        expected_revision: MetadataRevision,
        manifest_digest: Digest,
        plan_digest: Digest,
    ) -> Result<PlanReference, JobError> {
        self.require_state(JobState::Reviewing, JobOperation::CreatePlan)?;
        self.require_revision(expected_revision)?;
        if self.approved_revision != Some(self.metadata_revision) {
            return Err(JobError::RevisionNotApproved {
                actual: self.metadata_revision,
            });
        }

        let plan = PlanReference {
            metadata_revision: self.metadata_revision,
            manifest_digest,
            plan_digest,
        };
        self.plan = Some(plan);
        self.verification = None;
        self.state = JobState::Planned;
        Ok(plan)
    }

    pub fn begin_execution(&mut self, plan_digest: Digest) -> Result<(), JobError> {
        self.require_state(JobState::Planned, JobOperation::BeginExecution)?;
        self.require_plan(plan_digest)?;
        self.state = JobState::Executing;
        Ok(())
    }

    pub fn begin_verification(&mut self, plan_digest: Digest) -> Result<(), JobError> {
        self.require_state(JobState::Executing, JobOperation::BeginVerification)?;
        self.require_plan(plan_digest)?;
        self.state = JobState::Verifying;
        Ok(())
    }

    pub fn accept_verification(
        &mut self,
        plan_digest: Digest,
        receipt_digest: Digest,
    ) -> Result<VerificationReference, JobError> {
        self.require_state(JobState::Verifying, JobOperation::AcceptVerification)?;
        self.require_plan(plan_digest)?;

        let verification = VerificationReference {
            plan_digest,
            receipt_digest,
        };
        self.verification = Some(verification);
        self.state = JobState::ReadyToCommit;
        Ok(verification)
    }

    pub fn begin_commit(
        &mut self,
        plan_digest: Digest,
        verification_digest: Digest,
    ) -> Result<(), JobError> {
        self.require_state(JobState::ReadyToCommit, JobOperation::BeginCommit)?;
        self.require_plan(plan_digest)?;
        self.require_verification(plan_digest, verification_digest)?;
        self.state = JobState::Committing;
        Ok(())
    }

    pub fn publish(
        &mut self,
        plan_digest: Digest,
        verification_digest: Digest,
    ) -> Result<(), JobError> {
        self.require_state(JobState::Committing, JobOperation::Publish)?;
        self.require_plan(plan_digest)?;
        self.require_verification(plan_digest, verification_digest)?;
        self.state = JobState::Published;
        Ok(())
    }

    pub fn quarantine(&mut self) -> Result<(), JobError> {
        // Once commit begins, recovery must move forward from durable receipts;
        // changing the job to a generic quarantine state would hide that fact.
        if matches!(
            self.state,
            JobState::Committing | JobState::Published | JobState::Cancelled
        ) {
            return Err(self.invalid_transition(JobOperation::Quarantine));
        }
        self.state = JobState::Quarantined;
        Ok(())
    }

    pub fn cancel(&mut self) -> Result<(), JobError> {
        if matches!(
            self.state,
            JobState::Committing | JobState::Published | JobState::Cancelled
        ) {
            return Err(self.invalid_transition(JobOperation::Cancel));
        }
        self.state = JobState::Cancelled;
        Ok(())
    }

    fn require_state(&self, expected: JobState, operation: JobOperation) -> Result<(), JobError> {
        if self.state == expected {
            Ok(())
        } else {
            Err(self.invalid_transition(operation))
        }
    }

    fn require_revision(&self, expected: MetadataRevision) -> Result<(), JobError> {
        if self.metadata_revision == expected {
            Ok(())
        } else {
            Err(JobError::RevisionConflict {
                expected,
                actual: self.metadata_revision,
            })
        }
    }

    fn require_plan(&self, actual: Digest) -> Result<PlanReference, JobError> {
        let plan = self.plan.ok_or(JobError::MissingPlan)?;
        if plan.plan_digest == actual {
            Ok(plan)
        } else {
            Err(JobError::PlanMismatch {
                expected: plan.plan_digest,
                actual,
            })
        }
    }

    fn require_verification(
        &self,
        plan_digest: Digest,
        actual: Digest,
    ) -> Result<VerificationReference, JobError> {
        let verification = self.verification.ok_or(JobError::MissingVerification)?;
        if verification.plan_digest != plan_digest {
            return Err(JobError::PlanMismatch {
                expected: verification.plan_digest,
                actual: plan_digest,
            });
        }
        if verification.receipt_digest != actual {
            return Err(JobError::VerificationMismatch {
                expected: verification.receipt_digest,
                actual,
            });
        }
        Ok(verification)
    }

    fn invalid_transition(&self, operation: JobOperation) -> JobError {
        JobError::InvalidTransition {
            state: self.state,
            operation,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(byte: u8) -> Digest {
        Digest::new([byte; Digest::LENGTH])
    }

    fn ready_to_commit_job() -> IngestJob {
        let mut job = IngestJob::new(Uuid::new_v4());
        job.begin_review().unwrap();
        job.approve_revision(MetadataRevision::INITIAL).unwrap();
        job.create_plan(MetadataRevision::INITIAL, digest(1), digest(2))
            .unwrap();
        job.begin_execution(digest(2)).unwrap();
        job.begin_verification(digest(2)).unwrap();
        job.accept_verification(digest(2), digest(3)).unwrap();
        job
    }

    #[test]
    fn approved_job_reaches_published_with_matching_receipts() {
        let mut job = ready_to_commit_job();

        job.begin_commit(digest(2), digest(3)).unwrap();
        job.publish(digest(2), digest(3)).unwrap();

        assert_eq!(job.state(), JobState::Published);
    }

    #[test]
    fn invalid_transitions_are_rejected_at_the_domain_boundary() {
        let mut job = IngestJob::new(Uuid::new_v4());

        let error = job
            .create_plan(MetadataRevision::INITIAL, digest(1), digest(2))
            .unwrap_err();
        assert_eq!(
            error,
            JobError::InvalidTransition {
                state: JobState::Created,
                operation: JobOperation::CreatePlan,
            }
        );

        job.begin_review().unwrap();
        assert_eq!(
            job.publish(digest(2), digest(3)).unwrap_err(),
            JobError::InvalidTransition {
                state: JobState::Reviewing,
                operation: JobOperation::Publish,
            }
        );
    }

    #[test]
    fn persisted_state_names_round_trip() {
        let states = [
            JobState::Created,
            JobState::Reviewing,
            JobState::Planned,
            JobState::Executing,
            JobState::Verifying,
            JobState::ReadyToCommit,
            JobState::Committing,
            JobState::Published,
            JobState::Quarantined,
            JobState::Cancelled,
        ];

        for state in states {
            assert_eq!(state.as_str().parse(), Ok(state));
        }
    }

    #[test]
    fn revising_metadata_invalidates_plan_and_verification() {
        let mut job = ready_to_commit_job();

        let revision = job.revise_metadata(MetadataRevision::INITIAL).unwrap();

        assert_eq!(revision.get(), 2);
        assert_eq!(job.state(), JobState::Reviewing);
        assert_eq!(job.approved_revision(), None);
        assert_eq!(job.plan(), None);
        assert_eq!(job.verification(), None);
    }

    #[test]
    fn stale_revision_and_receipt_hashes_are_rejected() {
        let mut job = IngestJob::new(Uuid::new_v4());
        job.begin_review().unwrap();
        job.revise_metadata(MetadataRevision::INITIAL).unwrap();

        assert_eq!(
            job.approve_revision(MetadataRevision::INITIAL).unwrap_err(),
            JobError::RevisionConflict {
                expected: MetadataRevision::INITIAL,
                actual: MetadataRevision::new(2).unwrap(),
            }
        );

        let mut job = ready_to_commit_job();
        assert_eq!(
            job.begin_commit(digest(9), digest(3)).unwrap_err(),
            JobError::PlanMismatch {
                expected: digest(2),
                actual: digest(9),
            }
        );
        assert_eq!(
            job.begin_commit(digest(2), digest(9)).unwrap_err(),
            JobError::VerificationMismatch {
                expected: digest(3),
                actual: digest(9),
            }
        );
    }

    #[test]
    fn quarantine_keeps_diagnostics_but_blocks_direct_publish() {
        let mut job = ready_to_commit_job();
        let plan = job.plan();
        let verification = job.verification();

        job.quarantine().unwrap();

        assert_eq!(job.state(), JobState::Quarantined);
        assert_eq!(job.plan(), plan);
        assert_eq!(job.verification(), verification);
        assert!(matches!(
            job.publish(digest(2), digest(3)),
            Err(JobError::InvalidTransition {
                state: JobState::Quarantined,
                operation: JobOperation::Publish,
            })
        ));

        let mut committing = ready_to_commit_job();
        committing.begin_commit(digest(2), digest(3)).unwrap();
        assert!(matches!(
            committing.quarantine(),
            Err(JobError::InvalidTransition {
                state: JobState::Committing,
                operation: JobOperation::Quarantine,
            })
        ));
    }
}
