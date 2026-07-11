//! Persistence boundary for audio-ingestion jobs.
//!
//! GraphQL resolvers and background workers both use this repository so every
//! write receives the same optimistic-concurrency protection.

use anni_ingest::{
    Digest, IngestJob, IngestJobSnapshot, JobState, MetadataDraft, MetadataDraftSnapshot,
    MetadataError, MetadataRevision, SnapshotError,
};
use sea_orm::prelude::{DateTimeUtc, Uuid};
use sea_orm::{
    sea_query::{Expr, OnConflict},
    ActiveValue::NotSet,
    ActiveValue::Set,
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, TransactionTrait, TryInsertResult,
};
use sha2::{Digest as ShaDigest, Sha256};
use thiserror::Error;

use crate::entities::{
    helper::{now, timestamp},
    ingest_job, ingest_metadata_revision,
};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedMetadataDraft {
    draft: MetadataDraft,
    created_at: DateTimeUtc,
    updated_at: DateTimeUtc,
}

impl PersistedMetadataDraft {
    pub const fn draft(&self) -> &MetadataDraft {
        &self.draft
    }

    pub fn into_draft(self) -> MetadataDraft {
        self.draft
    }

    pub const fn created_at(&self) -> DateTimeUtc {
        self.created_at
    }

    pub const fn updated_at(&self) -> DateTimeUtc {
        self.updated_at
    }
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
    #[error(
        "ingest job {job_id} is at metadata revision {job_revision}, but the draft is revision {draft_revision}"
    )]
    MetadataRevisionMismatch {
        job_id: Uuid,
        job_revision: MetadataRevision,
        draft_revision: MetadataRevision,
    },
    #[error(
        "ingest job {job_id} cannot change metadata revision from {stored_revision} to {requested_revision} without a metadata document"
    )]
    MetadataRevisionRequiresDocument {
        job_id: Uuid,
        stored_revision: MetadataRevision,
        requested_revision: MetadataRevision,
    },
    #[error("metadata revision {revision} for ingest job {job_id} is frozen")]
    MetadataFrozen {
        job_id: Uuid,
        revision: MetadataRevision,
    },
    #[error("metadata revision {revision} for ingest job {job_id} does not exist")]
    MetadataNotFound {
        job_id: Uuid,
        revision: MetadataRevision,
    },
    #[error("could not serialize metadata revision {revision} for ingest job {job_id}: {source}")]
    SerializeMetadata {
        job_id: Uuid,
        revision: MetadataRevision,
        #[source]
        source: serde_json::Error,
    },
    #[error(
        "persisted metadata revision {revision} for ingest job {job_id} has an invalid SHA-256 length: {actual}"
    )]
    InvalidMetadataDigestLength {
        job_id: Uuid,
        revision: MetadataRevision,
        actual: usize,
    },
    #[error(
        "persisted metadata revision {revision} for ingest job {job_id} does not match its SHA-256"
    )]
    MetadataDigestMismatch {
        job_id: Uuid,
        revision: MetadataRevision,
    },
    #[error("persisted metadata revision {revision} for ingest job {job_id} is not valid JSON: {source}")]
    InvalidMetadataDocument {
        job_id: Uuid,
        revision: MetadataRevision,
        #[source]
        source: serde_json::Error,
    },
    #[error(
        "persisted metadata revision {revision} for ingest job {job_id} violates domain invariants: {source}"
    )]
    InvalidPersistedMetadata {
        job_id: Uuid,
        revision: MetadataRevision,
        #[source]
        source: MetadataError,
    },
    #[error(
        "persisted metadata row {row_revision} for ingest job {job_id} contains revision {document_revision}"
    )]
    MetadataDocumentRevisionMismatch {
        job_id: Uuid,
        row_revision: MetadataRevision,
        document_revision: MetadataRevision,
    },
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
        let transaction = self.database.begin().await?;
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
            .exec_without_returning(&transaction)
            .await?;

        match result {
            TryInsertResult::Inserted(1) => {
                insert_metadata_document(
                    &transaction,
                    snapshot.id(),
                    &MetadataDraft::new(snapshot.metadata_revision()),
                )
                .await?;
                transaction.commit().await?;
                self.get(job.id()).await?.ok_or_else(|| {
                    IngestRepositoryError::Database(DbErr::Custom(
                        "created ingest job could not be read back".to_owned(),
                    ))
                })
            }
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
        get_job(&self.database, job_id).await
    }

    pub async fn get_metadata_draft(
        &self,
        job_id: Uuid,
        revision: MetadataRevision,
    ) -> Result<Option<PersistedMetadataDraft>, IngestRepositoryError> {
        get_metadata_draft(&self.database, job_id, revision).await
    }

    pub async fn list_metadata_revisions(
        &self,
        job_id: Uuid,
    ) -> Result<Vec<PersistedMetadataDraft>, IngestRepositoryError> {
        ingest_metadata_revision::Entity::find()
            .filter(ingest_metadata_revision::Column::JobId.eq(job_id))
            .order_by_desc(ingest_metadata_revision::Column::Revision)
            .all(&self.database)
            .await?
            .into_iter()
            .map(model_to_metadata_draft)
            .collect()
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
        compare_and_swap(&self.database, job, expected, false).await
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

    /// Atomically persist a job change and its current metadata document.
    /// Historical revision rows are never selected by this method because the
    /// draft revision must match the job aggregate's current revision.
    pub async fn save_with_metadata(
        &self,
        versioned: &mut VersionedIngestJob,
        draft: &MetadataDraft,
    ) -> Result<PersistedMetadataDraft, IngestRepositoryError> {
        let job_id = versioned.job().id();
        let job_revision = versioned.job().metadata_revision();
        if draft.revision() != job_revision {
            return Err(IngestRepositoryError::MetadataRevisionMismatch {
                job_id,
                job_revision,
                draft_revision: draft.revision(),
            });
        }
        if versioned.job().state() != JobState::Reviewing
            || versioned.job().approved_revision() == Some(job_revision)
        {
            return Err(IngestRepositoryError::MetadataFrozen {
                job_id,
                revision: job_revision,
            });
        }

        let transaction = self.database.begin().await?;
        compare_and_swap(&transaction, versioned.job(), versioned.row_version(), true).await?;
        upsert_metadata_document(&transaction, job_id, draft).await?;
        transaction.commit().await?;

        *versioned = self
            .get(job_id)
            .await?
            .ok_or(IngestRepositoryError::NotFound { job_id })?;
        self.get_metadata_draft(job_id, draft.revision())
            .await?
            .ok_or_else(|| {
                IngestRepositoryError::Database(DbErr::Custom(
                    "saved metadata document could not be read back".to_owned(),
                ))
            })
    }
}

async fn get_job<C: ConnectionTrait>(
    connection: &C,
    job_id: Uuid,
) -> Result<Option<VersionedIngestJob>, IngestRepositoryError> {
    ingest_job::Entity::find()
        .filter(ingest_job::Column::JobId.eq(job_id))
        .one(connection)
        .await?
        .map(model_to_versioned)
        .transpose()
}

async fn compare_and_swap<C: ConnectionTrait>(
    connection: &C,
    job: &IngestJob,
    expected: RowVersion,
    allow_metadata_revision_change: bool,
) -> Result<RowVersion, IngestRepositoryError> {
    if !allow_metadata_revision_change {
        let current = get_job(connection, job.id())
            .await?
            .ok_or(IngestRepositoryError::NotFound { job_id: job.id() })?;
        if current.row_version() != expected {
            return Err(IngestRepositoryError::ConcurrentModification {
                job_id: job.id(),
                expected,
                actual: current.row_version(),
            });
        }
        if current.job().metadata_revision() != job.metadata_revision() {
            return Err(IngestRepositoryError::MetadataRevisionRequiresDocument {
                job_id: job.id(),
                stored_revision: current.job().metadata_revision(),
                requested_revision: job.metadata_revision(),
            });
        }
    }

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
        .exec(connection)
        .await?;

    if result.rows_affected == 1 {
        return Ok(next);
    }

    match get_job(connection, job.id()).await? {
        Some(current) => Err(IngestRepositoryError::ConcurrentModification {
            job_id: job.id(),
            expected,
            actual: current.row_version,
        }),
        None => Err(IngestRepositoryError::NotFound { job_id: job.id() }),
    }
}

async fn get_metadata_draft<C: ConnectionTrait>(
    connection: &C,
    job_id: Uuid,
    revision: MetadataRevision,
) -> Result<Option<PersistedMetadataDraft>, IngestRepositoryError> {
    ingest_metadata_revision::Entity::find()
        .filter(ingest_metadata_revision::Column::JobId.eq(job_id))
        .filter(
            ingest_metadata_revision::Column::Revision.eq(revision_to_i64(
                job_id,
                revision,
                "metadata_revision",
            )?),
        )
        .one(connection)
        .await?
        .map(model_to_metadata_draft)
        .transpose()
}

async fn insert_metadata_document<C: ConnectionTrait>(
    connection: &C,
    job_id: Uuid,
    draft: &MetadataDraft,
) -> Result<(), IngestRepositoryError> {
    ingest_metadata_revision::Entity::insert(metadata_active_model(job_id, draft)?)
        .exec_without_returning(connection)
        .await?;
    Ok(())
}

async fn upsert_metadata_document<C: ConnectionTrait>(
    connection: &C,
    job_id: Uuid,
    draft: &MetadataDraft,
) -> Result<(), IngestRepositoryError> {
    ingest_metadata_revision::Entity::insert(metadata_active_model(job_id, draft)?)
        .on_conflict(
            OnConflict::columns([
                ingest_metadata_revision::Column::JobId,
                ingest_metadata_revision::Column::Revision,
            ])
            .update_columns([
                ingest_metadata_revision::Column::Document,
                ingest_metadata_revision::Column::DocumentSha256,
                ingest_metadata_revision::Column::UpdatedAt,
            ])
            .to_owned(),
        )
        .exec_without_returning(connection)
        .await?;
    Ok(())
}

fn metadata_active_model(
    job_id: Uuid,
    draft: &MetadataDraft,
) -> Result<ingest_metadata_revision::ActiveModel, IngestRepositoryError> {
    let revision = draft.revision();
    let document = serde_json::to_string(&draft.snapshot()).map_err(|source| {
        IngestRepositoryError::SerializeMetadata {
            job_id,
            revision,
            source,
        }
    })?;
    let document_sha256 = sha256(document.as_bytes());

    Ok(ingest_metadata_revision::ActiveModel {
        id: NotSet,
        job_id: Set(job_id),
        revision: Set(revision_to_i64(job_id, revision, "metadata_revision")?),
        document: Set(document),
        document_sha256: Set(document_sha256.as_bytes().to_vec()),
        created_at: NotSet,
        updated_at: Set(now()),
    })
}

fn model_to_metadata_draft(
    model: ingest_metadata_revision::Model,
) -> Result<PersistedMetadataDraft, IngestRepositoryError> {
    let job_id = model.job_id;
    let revision = revision_from_i64(job_id, model.revision, "metadata_revision")?;
    let digest_length = model.document_sha256.len();
    let stored_digest = <[u8; Digest::LENGTH]>::try_from(model.document_sha256)
        .map(Digest::new)
        .map_err(|_| IngestRepositoryError::InvalidMetadataDigestLength {
            job_id,
            revision,
            actual: digest_length,
        })?;
    if sha256(model.document.as_bytes()) != stored_digest {
        return Err(IngestRepositoryError::MetadataDigestMismatch { job_id, revision });
    }

    let snapshot: MetadataDraftSnapshot =
        serde_json::from_str(&model.document).map_err(|source| {
            IngestRepositoryError::InvalidMetadataDocument {
                job_id,
                revision,
                source,
            }
        })?;
    let draft = MetadataDraft::restore(snapshot).map_err(|source| {
        IngestRepositoryError::InvalidPersistedMetadata {
            job_id,
            revision,
            source,
        }
    })?;
    if draft.revision() != revision {
        return Err(IngestRepositoryError::MetadataDocumentRevisionMismatch {
            job_id,
            row_revision: revision,
            document_revision: draft.revision(),
        });
    }

    Ok(PersistedMetadataDraft {
        draft,
        created_at: timestamp(model.created_at),
        updated_at: timestamp(model.updated_at),
    })
}

fn sha256(value: &[u8]) -> Digest {
    let mut hasher = Sha256::new();
    hasher.update(value);
    Digest::new(hasher.finalize().into())
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
