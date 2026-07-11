//! Persistence boundary for audio-ingestion jobs.
//!
//! GraphQL resolvers and background workers both use this repository so every
//! write receives the same optimistic-concurrency protection.

use anni_ingest::{
    Digest, IngestJob, IngestJobSnapshot, JobState, MetadataRevision, SnapshotError,
};
use sea_orm::prelude::{DateTimeUtc, Uuid};
use sea_orm::{
    sea_query::{Expr, OnConflict},
    ActiveValue::NotSet,
    ActiveValue::Set,
    ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    TryInsertResult,
};
use thiserror::Error;

use crate::entities::{helper::timestamp, ingest_job};

mod service;

pub use service::{IngestCommand, IngestJobEvent, IngestService, IngestServiceError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RowVersion(u64);

impl RowVersion {
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

    fn as_i64(self, job_id: Uuid) -> Result<i64, IngestRepositoryError> {
        i64::try_from(self.0).map_err(|_| IngestRepositoryError::NumericOutOfRange {
            job_id,
            field: "row_version",
        })
    }

    fn next(self, job_id: Uuid) -> Result<Self, IngestRepositoryError> {
        self.0
            .checked_add(1)
            .filter(|value| i64::try_from(*value).is_ok())
            .map(Self)
            .ok_or(IngestRepositoryError::NumericOutOfRange {
                job_id,
                field: "row_version",
            })
    }
}

impl std::fmt::Display for RowVersion {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionedIngestJob {
    job: IngestJob,
    row_version: RowVersion,
    created_at: DateTimeUtc,
    updated_at: DateTimeUtc,
}

impl VersionedIngestJob {
    pub const fn job(&self) -> &IngestJob {
        &self.job
    }

    pub fn job_mut(&mut self) -> &mut IngestJob {
        &mut self.job
    }

    pub const fn row_version(&self) -> RowVersion {
        self.row_version
    }

    pub const fn created_at(&self) -> DateTimeUtc {
        self.created_at
    }

    pub const fn updated_at(&self) -> DateTimeUtc {
        self.updated_at
    }

    pub fn into_job(self) -> IngestJob {
        self.job
    }
}

#[derive(Debug, Error)]
pub enum IngestRepositoryError {
    #[error("ingest job {job_id} already exists")]
    AlreadyExists { job_id: Uuid },
    #[error("ingest job {job_id} does not exist")]
    NotFound { job_id: Uuid },
    #[error(
        "ingest job {job_id} changed concurrently: expected row version {expected}, actual {actual}"
    )]
    ConcurrentModification {
        job_id: Uuid,
        expected: RowVersion,
        actual: RowVersion,
    },
    #[error("persisted ingest job {job_id} has an unknown state: {state}")]
    InvalidState { job_id: Uuid, state: String },
    #[error("persisted ingest job {job_id} is invalid: {source}")]
    InvalidPersistedJob {
        job_id: Uuid,
        #[source]
        source: SnapshotError,
    },
    #[error("persisted ingest job {job_id} has an invalid {field} length: {actual}")]
    InvalidDigestLength {
        job_id: Uuid,
        field: &'static str,
        actual: usize,
    },
    #[error("persisted ingest job {job_id} has an out-of-range {field}")]
    NumericOutOfRange { job_id: Uuid, field: &'static str },
    #[error(transparent)]
    Database(#[from] DbErr),
}

#[derive(Clone)]
pub struct IngestJobRepository {
    database: DatabaseConnection,
}

impl IngestJobRepository {
    pub fn new(database: DatabaseConnection) -> Self {
        Self { database }
    }

    pub async fn create(
        &self,
        job: &IngestJob,
    ) -> Result<VersionedIngestJob, IngestRepositoryError> {
        let snapshot = job.snapshot();
        let active_model = ingest_job::ActiveModel {
            id: NotSet,
            job_id: Set(snapshot.id()),
            state: Set(snapshot.state().as_str().to_owned()),
            metadata_revision: Set(revision_to_i64(
                snapshot.id(),
                snapshot.metadata_revision(),
                "metadata_revision",
            )?),
            approved_revision: Set(optional_revision_to_i64(
                snapshot.id(),
                snapshot.approved_revision(),
                "approved_revision",
            )?),
            manifest_digest: Set(digest_bytes(snapshot.manifest_digest())),
            plan_digest: Set(digest_bytes(snapshot.plan_digest())),
            verification_digest: Set(digest_bytes(snapshot.verification_digest())),
            row_version: Set(RowVersion::INITIAL.as_i64(snapshot.id())?),
            created_at: NotSet,
            updated_at: NotSet,
        };

        let result = ingest_job::Entity::insert(active_model)
            .on_conflict(
                OnConflict::column(ingest_job::Column::JobId)
                    .do_nothing()
                    .to_owned(),
            )
            .do_nothing()
            .exec_without_returning(&self.database)
            .await?;

        match result {
            TryInsertResult::Inserted(1) => self.get(job.id()).await?.ok_or_else(|| {
                IngestRepositoryError::Database(DbErr::Custom(
                    "created ingest job could not be read back".to_owned(),
                ))
            }),
            TryInsertResult::Conflicted | TryInsertResult::Inserted(0) => {
                Err(IngestRepositoryError::AlreadyExists { job_id: job.id() })
            }
            TryInsertResult::Inserted(_) | TryInsertResult::Empty => {
                Err(IngestRepositoryError::Database(DbErr::Custom(
                    "ingest job insert affected an unexpected number of rows".to_owned(),
                )))
            }
        }
    }

    pub async fn get(
        &self,
        job_id: Uuid,
    ) -> Result<Option<VersionedIngestJob>, IngestRepositoryError> {
        ingest_job::Entity::find()
            .filter(ingest_job::Column::JobId.eq(job_id))
            .one(&self.database)
            .await?
            .map(model_to_versioned)
            .transpose()
    }

    pub async fn list(
        &self,
        state: Option<JobState>,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<VersionedIngestJob>, IngestRepositoryError> {
        let mut query = ingest_job::Entity::find();
        if let Some(state) = state {
            query = query.filter(ingest_job::Column::State.eq(state.as_str()));
        }

        query
            .order_by_desc(ingest_job::Column::Id)
            .limit(limit)
            .offset(offset)
            .all(&self.database)
            .await?
            .into_iter()
            .map(model_to_versioned)
            .collect()
    }

    /// Atomically replace the durable snapshot only if no other actor has
    /// written the row since `expected` was loaded.
    pub async fn compare_and_swap(
        &self,
        job: &IngestJob,
        expected: RowVersion,
    ) -> Result<RowVersion, IngestRepositoryError> {
        let snapshot = job.snapshot();
        let expected_i64 = expected.as_i64(job.id())?;
        let next = expected.next(job.id())?;

        let result = ingest_job::Entity::update_many()
            .col_expr(
                ingest_job::Column::State,
                Expr::value(snapshot.state().as_str()),
            )
            .col_expr(
                ingest_job::Column::MetadataRevision,
                Expr::value(revision_to_i64(
                    snapshot.id(),
                    snapshot.metadata_revision(),
                    "metadata_revision",
                )?),
            )
            .col_expr(
                ingest_job::Column::ApprovedRevision,
                Expr::value(optional_revision_to_i64(
                    snapshot.id(),
                    snapshot.approved_revision(),
                    "approved_revision",
                )?),
            )
            .col_expr(
                ingest_job::Column::ManifestDigest,
                Expr::value(digest_bytes(snapshot.manifest_digest())),
            )
            .col_expr(
                ingest_job::Column::PlanDigest,
                Expr::value(digest_bytes(snapshot.plan_digest())),
            )
            .col_expr(
                ingest_job::Column::VerificationDigest,
                Expr::value(digest_bytes(snapshot.verification_digest())),
            )
            .col_expr(
                ingest_job::Column::RowVersion,
                Expr::value(next.as_i64(job.id())?),
            )
            .col_expr(
                ingest_job::Column::UpdatedAt,
                Expr::current_timestamp().into(),
            )
            .filter(ingest_job::Column::JobId.eq(job.id()))
            .filter(ingest_job::Column::RowVersion.eq(expected_i64))
            .exec(&self.database)
            .await?;

        if result.rows_affected == 1 {
            return Ok(next);
        }

        match self.get(job.id()).await? {
            Some(current) => Err(IngestRepositoryError::ConcurrentModification {
                job_id: job.id(),
                expected,
                actual: current.row_version,
            }),
            None => Err(IngestRepositoryError::NotFound { job_id: job.id() }),
        }
    }

    pub async fn save(
        &self,
        versioned: &mut VersionedIngestJob,
    ) -> Result<(), IngestRepositoryError> {
        self.compare_and_swap(&versioned.job, versioned.row_version)
            .await?;
        *versioned =
            self.get(versioned.job.id())
                .await?
                .ok_or(IngestRepositoryError::NotFound {
                    job_id: versioned.job.id(),
                })?;
        Ok(())
    }
}

fn model_to_versioned(
    model: ingest_job::Model,
) -> Result<VersionedIngestJob, IngestRepositoryError> {
    let job_id = model.job_id;
    let state = model
        .state
        .parse()
        .map_err(|_| IngestRepositoryError::InvalidState {
            job_id,
            state: model.state,
        })?;
    let metadata_revision =
        revision_from_i64(job_id, model.metadata_revision, "metadata_revision")?;
    let approved_revision =
        optional_revision_from_i64(job_id, model.approved_revision, "approved_revision")?;
    let snapshot = IngestJobSnapshot::new(
        job_id,
        state,
        metadata_revision,
        approved_revision,
        digest_from_bytes(job_id, "manifest_digest", model.manifest_digest)?,
        digest_from_bytes(job_id, "plan_digest", model.plan_digest)?,
        digest_from_bytes(job_id, "verification_digest", model.verification_digest)?,
    );
    let job = IngestJob::restore(snapshot)
        .map_err(|source| IngestRepositoryError::InvalidPersistedJob { job_id, source })?;
    let row_version = RowVersion::new(u64::try_from(model.row_version).map_err(|_| {
        IngestRepositoryError::NumericOutOfRange {
            job_id,
            field: "row_version",
        }
    })?)
    .ok_or(IngestRepositoryError::NumericOutOfRange {
        job_id,
        field: "row_version",
    })?;

    Ok(VersionedIngestJob {
        job,
        row_version,
        created_at: timestamp(model.created_at),
        updated_at: timestamp(model.updated_at),
    })
}

fn revision_to_i64(
    job_id: Uuid,
    revision: MetadataRevision,
    field: &'static str,
) -> Result<i64, IngestRepositoryError> {
    i64::try_from(revision.get())
        .map_err(|_| IngestRepositoryError::NumericOutOfRange { job_id, field })
}

fn optional_revision_to_i64(
    job_id: Uuid,
    revision: Option<MetadataRevision>,
    field: &'static str,
) -> Result<Option<i64>, IngestRepositoryError> {
    revision
        .map(|revision| revision_to_i64(job_id, revision, field))
        .transpose()
}

fn revision_from_i64(
    job_id: Uuid,
    value: i64,
    field: &'static str,
) -> Result<MetadataRevision, IngestRepositoryError> {
    let value = u64::try_from(value)
        .map_err(|_| IngestRepositoryError::NumericOutOfRange { job_id, field })?;
    MetadataRevision::new(value).ok_or(IngestRepositoryError::NumericOutOfRange { job_id, field })
}

fn optional_revision_from_i64(
    job_id: Uuid,
    value: Option<i64>,
    field: &'static str,
) -> Result<Option<MetadataRevision>, IngestRepositoryError> {
    value
        .map(|value| revision_from_i64(job_id, value, field))
        .transpose()
}

fn digest_bytes(digest: Option<Digest>) -> Option<Vec<u8>> {
    digest.map(|digest| digest.as_bytes().to_vec())
}

fn digest_from_bytes(
    job_id: Uuid,
    field: &'static str,
    bytes: Option<Vec<u8>>,
) -> Result<Option<Digest>, IngestRepositoryError> {
    bytes
        .map(|bytes| {
            let actual = bytes.len();
            <[u8; Digest::LENGTH]>::try_from(bytes)
                .map(Digest::new)
                .map_err(|_| IngestRepositoryError::InvalidDigestLength {
                    job_id,
                    field,
                    actual,
                })
        })
        .transpose()
}
