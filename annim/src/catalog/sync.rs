//! Durable synchronization of artist discographies from external catalogs.
//!
//! A sync run never treats a remote response as canonical metadata. Instead,
//! it stores the exact response as an append-only observation. Parsed data is
//! stored beside the response so parser changes are reviewable, while the raw
//! SHA-256 makes accidental corruption detectable.
//!
//! Public snapshots deliberately omit `configuration_document` and
//! `secret_ref`. Those values are worker-only inputs and must not leak through
//! a future GraphQL query merely because a source was listed.

use std::{fmt, str::FromStr};

use anni_catalog::{CatalogSourceKind, SyncRunStatus};
use anni_ingest::Digest;
use sea_orm::{
    prelude::{DateTimeUtc, Uuid},
    sea_query::Expr,
    ActiveValue::{NotSet, Set},
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, QueryOrder,
    TransactionTrait,
};
use sha2::{Digest as ShaDigest, Sha256};
use thiserror::Error;

use super::CatalogRowVersion;
use crate::entities::{
    catalog_artist, catalog_source, catalog_source_release, catalog_source_release_revision,
    catalog_sync_run,
    helper::{now, timestamp},
};

const MAX_SYNC_ERROR_CHARS: usize = 512;

#[cfg(feature = "postgres")]
type PersistedTimestamp = chrono::NaiveDateTime;
#[cfg(all(feature = "sqlite", not(feature = "postgres")))]
type PersistedTimestamp = DateTimeUtc;

#[derive(Clone, PartialEq, Eq)]
pub struct CatalogSourceSnapshot {
    pub source_id: Uuid,
    pub artist_id: Uuid,
    pub kind: CatalogSourceKind,
    pub locator: String,
    pub storefront: Option<String>,
    pub locale: Option<String>,
    pub enabled: bool,
    pub row_version: CatalogRowVersion,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

impl fmt::Debug for CatalogSourceSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CatalogSourceSnapshot")
            .field("source_id", &self.source_id)
            .field("artist_id", &self.artist_id)
            .field("kind", &self.kind)
            .field("locator", &"[REDACTED]")
            .field("storefront", &self.storefront)
            .field("locale", &self.locale)
            .field("enabled", &self.enabled)
            .field("row_version", &self.row_version)
            .field("created_at", &self.created_at)
            .field("updated_at", &self.updated_at)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct NewCatalogSource {
    pub source_id: Option<Uuid>,
    pub artist_id: Uuid,
    pub kind: CatalogSourceKind,
    pub locator: String,
    pub storefront: Option<String>,
    pub locale: Option<String>,
    /// Adapter configuration. This is accepted at the command boundary but is
    /// intentionally absent from [`CatalogSourceSnapshot`].
    pub configuration_document: Option<String>,
    /// Reference into the worker's secret store, never the secret itself.
    pub secret_ref: Option<String>,
}

impl fmt::Debug for NewCatalogSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NewCatalogSource")
            .field("source_id", &self.source_id)
            .field("artist_id", &self.artist_id)
            .field("kind", &self.kind)
            .field("locator", &"[REDACTED]")
            .field("storefront", &self.storefront)
            .field("locale", &self.locale)
            .field(
                "configuration_document",
                &self.configuration_document.as_ref().map(|_| "[REDACTED]"),
            )
            .field(
                "secret_ref",
                &self.secret_ref.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct CatalogSyncRunSnapshot {
    pub run_id: Uuid,
    pub source_id: Uuid,
    pub status: SyncRunStatus,
    pub requested_cursor: Option<String>,
    pub result_cursor: Option<String>,
    pub observed_count: u32,
    pub error_message: Option<String>,
    pub row_version: CatalogRowVersion,
    pub created_at: DateTimeUtc,
    pub started_at: Option<DateTimeUtc>,
    pub finished_at: Option<DateTimeUtc>,
}

impl fmt::Debug for CatalogSyncRunSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CatalogSyncRunSnapshot")
            .field("run_id", &self.run_id)
            .field("source_id", &self.source_id)
            .field("status", &self.status)
            .field(
                "requested_cursor",
                &self.requested_cursor.as_ref().map(|_| "[REDACTED]"),
            )
            .field(
                "result_cursor",
                &self.result_cursor.as_ref().map(|_| "[REDACTED]"),
            )
            .field("observed_count", &self.observed_count)
            .field(
                "error_message",
                &self.error_message.as_ref().map(|_| "[REDACTED]"),
            )
            .field("row_version", &self.row_version)
            .field("created_at", &self.created_at)
            .field("started_at", &self.started_at)
            .field("finished_at", &self.finished_at)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct NewCatalogSyncRun {
    pub run_id: Option<Uuid>,
    pub source_id: Uuid,
    pub requested_cursor: Option<String>,
}

impl fmt::Debug for NewCatalogSyncRun {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NewCatalogSyncRun")
            .field("run_id", &self.run_id)
            .field("source_id", &self.source_id)
            .field(
                "requested_cursor",
                &self.requested_cursor.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

/// One exact release observation produced by a source adapter.
///
/// No Unicode normalization or whitespace rewriting occurs here: both
/// documents are hashed and persisted byte-for-byte as supplied.
#[derive(Clone, PartialEq, Eq)]
pub struct CatalogReleaseObservation {
    pub external_release_id: String,
    pub source_url: String,
    pub raw_document: String,
    pub parsed_document: String,
}

impl fmt::Debug for CatalogReleaseObservation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CatalogReleaseObservation")
            .field("external_release_id", &self.external_release_id)
            .field("source_url", &"[REDACTED]")
            .field("raw_document_bytes", &self.raw_document.len())
            .field("parsed_document_bytes", &self.parsed_document.len())
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct CatalogObservationSnapshot {
    pub source_release_id: Uuid,
    pub external_release_id: String,
    pub source_url: String,
    pub revision: u64,
    pub raw_sha256: Digest,
    pub first_seen_at: DateTimeUtc,
    pub last_seen_at: DateTimeUtc,
    pub not_seen_since: Option<DateTimeUtc>,
    pub row_version: CatalogRowVersion,
}

impl fmt::Debug for CatalogObservationSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CatalogObservationSnapshot")
            .field("source_release_id", &self.source_release_id)
            .field("external_release_id", &self.external_release_id)
            .field("source_url", &"[REDACTED]")
            .field("revision", &self.revision)
            .field("raw_sha256", &self.raw_sha256)
            .field("first_seen_at", &self.first_seen_at)
            .field("last_seen_at", &self.last_seen_at)
            .field("not_seen_since", &self.not_seen_since)
            .field("row_version", &self.row_version)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedCatalogObservation {
    pub run: CatalogSyncRunSnapshot,
    pub observation: CatalogObservationSnapshot,
    /// False when the exact raw and parsed documents already formed the latest
    /// revision. Seeing the same release in a later run still clears absence
    /// and advances the run, but does not duplicate evidence.
    pub revision_appended: bool,
    /// False for duplicate adapter output within the same run. This prevents a
    /// retry from inflating `observed_count`.
    pub first_observation_in_run: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub enum FinishCatalogSyncRun {
    Succeeded { result_cursor: Option<String> },
    Failed { error_message: String },
    Cancelled { message: Option<String> },
}

impl fmt::Debug for FinishCatalogSyncRun {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Succeeded { result_cursor } => formatter
                .debug_struct("Succeeded")
                .field(
                    "result_cursor",
                    &result_cursor.as_ref().map(|_| "[REDACTED]"),
                )
                .finish(),
            Self::Failed { .. } => formatter
                .debug_struct("Failed")
                .field("error_message", &"[REDACTED]")
                .finish(),
            Self::Cancelled { message } => formatter
                .debug_struct("Cancelled")
                .field("message", &message.as_ref().map(|_| "[REDACTED]"))
                .finish(),
        }
    }
}

impl FinishCatalogSyncRun {
    fn status(&self) -> SyncRunStatus {
        match self {
            Self::Succeeded { .. } => SyncRunStatus::Succeeded,
            Self::Failed { .. } => SyncRunStatus::Failed,
            Self::Cancelled { .. } => SyncRunStatus::Cancelled,
        }
    }

    fn result_cursor(&self) -> Option<&str> {
        match self {
            Self::Succeeded { result_cursor } => result_cursor.as_deref(),
            Self::Failed { .. } | Self::Cancelled { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinishedCatalogSyncRun {
    pub run: CatalogSyncRunSnapshot,
    /// Number of releases whose first absence marker was set by this run.
    /// Releases already absent retain their original `not_seen_since` date.
    pub newly_not_seen: u32,
}

#[derive(Debug, Error)]
pub enum CatalogSyncError {
    #[error("catalog artist {artist_id} does not exist")]
    ArtistNotFound { artist_id: Uuid },
    #[error("catalog source {source_id} already exists")]
    SourceAlreadyExists { source_id: Uuid },
    #[error("catalog source identity already exists for artist {artist_id}: {kind}")]
    SourceIdentityAlreadyExists {
        artist_id: Uuid,
        kind: CatalogSourceKind,
    },
    #[error("catalog source {source_id} does not exist")]
    SourceNotFound { source_id: Uuid },
    #[error("catalog source {source_id} is disabled")]
    SourceDisabled { source_id: Uuid },
    #[error("catalog source {source_id} already has a running or contending sync run")]
    SourceBusy { source_id: Uuid },
    #[error("catalog sync run {run_id} already exists")]
    RunAlreadyExists { run_id: Uuid },
    #[error("catalog sync run {run_id} does not exist")]
    RunNotFound { run_id: Uuid },
    #[error(
        "catalog sync run {run_id} changed concurrently: expected {expected}, actual {actual}"
    )]
    RunConflict {
        run_id: Uuid,
        expected: CatalogRowVersion,
        actual: CatalogRowVersion,
    },
    #[error("catalog sync run {run_id} cannot transition from {from} to {to}")]
    InvalidRunTransition {
        run_id: Uuid,
        from: SyncRunStatus,
        to: SyncRunStatus,
    },
    #[error("catalog sync run {run_id} is {status}, not running")]
    RunNotRunning { run_id: Uuid, status: SyncRunStatus },
    #[error("catalog source release {source_release_id} changed concurrently")]
    ObservationConflict { source_release_id: Uuid },
    #[error("invalid catalog sync {field}: {message}")]
    InvalidInput {
        field: &'static str,
        message: &'static str,
    },
    #[error("persisted {entity} {id} contains invalid {field}: {value}")]
    InvalidPersistedValue {
        entity: &'static str,
        id: Uuid,
        field: &'static str,
        value: String,
    },
    #[error("persisted source release {source_release_id} has no observation revision")]
    MissingObservationRevision { source_release_id: Uuid },
    #[error("persisted source release {source_release_id} revision {revision} has an invalid digest length: {actual}")]
    InvalidObservationDigestLength {
        source_release_id: Uuid,
        revision: u64,
        actual: usize,
    },
    #[error(
        "persisted source release {source_release_id} revision {revision} failed its SHA-256 check"
    )]
    ObservationDigestMismatch {
        source_release_id: Uuid,
        revision: u64,
    },
    #[error("{entity} {id} has an out-of-range {field}")]
    NumericOutOfRange {
        entity: &'static str,
        id: Uuid,
        field: &'static str,
    },
    #[error(transparent)]
    Database(#[from] DbErr),
}

#[derive(Clone)]
pub struct CatalogSyncService {
    database: DatabaseConnection,
}

impl CatalogSyncService {
    pub fn new(database: DatabaseConnection) -> Self {
        Self { database }
    }

    pub async fn create_source(
        &self,
        input: NewCatalogSource,
    ) -> Result<CatalogSourceSnapshot, CatalogSyncError> {
        require_non_empty("locator", &input.locator)?;
        let artist = get_artist_model(&self.database, input.artist_id)
            .await?
            .ok_or(CatalogSyncError::ArtistNotFound {
                artist_id: input.artist_id,
            })?;
        let source_id = input.source_id.unwrap_or_else(Uuid::new_v4);
        let identity_locator = input.locator.clone();
        let active_model = catalog_source::ActiveModel {
            id: NotSet,
            source_id: Set(source_id),
            artist_db_id: Set(artist.id),
            kind: Set(input.kind.as_str().to_owned()),
            locator: Set(input.locator),
            storefront: Set(input.storefront),
            locale: Set(input.locale),
            configuration_document: Set(input.configuration_document),
            secret_ref: Set(input.secret_ref),
            enabled: Set(true),
            row_version: Set(1),
            created_at: NotSet,
            updated_at: NotSet,
        };

        if let Err(error) = catalog_source::Entity::insert(active_model)
            .exec_without_returning(&self.database)
            .await
        {
            if get_source_model(&self.database, source_id).await?.is_some() {
                return Err(CatalogSyncError::SourceAlreadyExists { source_id });
            }
            if catalog_source::Entity::find()
                .filter(catalog_source::Column::ArtistDbId.eq(artist.id))
                .filter(catalog_source::Column::Kind.eq(input.kind.as_str()))
                .filter(catalog_source::Column::Locator.eq(&identity_locator))
                .one(&self.database)
                .await?
                .is_some()
            {
                return Err(CatalogSyncError::SourceIdentityAlreadyExists {
                    artist_id: input.artist_id,
                    kind: input.kind,
                });
            }
            return Err(error.into());
        }

        self.get_source(source_id)
            .await?
            .ok_or(CatalogSyncError::SourceNotFound { source_id })
    }

    pub async fn get_source(
        &self,
        source_id: Uuid,
    ) -> Result<Option<CatalogSourceSnapshot>, CatalogSyncError> {
        let Some(source) = get_source_model(&self.database, source_id).await? else {
            return Ok(None);
        };
        let artist = catalog_artist::Entity::find_by_id(source.artist_db_id)
            .one(&self.database)
            .await?
            .ok_or_else(|| {
                CatalogSyncError::Database(DbErr::Custom(format!(
                    "catalog source {source_id} references a missing artist"
                )))
            })?;
        source_model_to_snapshot(source, artist.artist_id).map(Some)
    }

    /// Lists the configured catalog sources for one artist in a stable order.
    /// Adapter configuration, secret references, and observation documents
    /// remain private because they are not part of the returned snapshot.
    pub async fn list_sources_for_artist(
        &self,
        artist_id: Uuid,
    ) -> Result<Vec<CatalogSourceSnapshot>, CatalogSyncError> {
        let artist = get_artist_model(&self.database, artist_id)
            .await?
            .ok_or(CatalogSyncError::ArtistNotFound { artist_id })?;
        catalog_source::Entity::find()
            .filter(catalog_source::Column::ArtistDbId.eq(artist.id))
            .order_by_asc(catalog_source::Column::Kind)
            .order_by_asc(catalog_source::Column::CreatedAt)
            .all(&self.database)
            .await?
            .into_iter()
            .map(|source| source_model_to_snapshot(source, artist_id))
            .collect()
    }

    /// Queue a run. A worker must subsequently claim it with the returned row
    /// version before it may write observations.
    pub async fn start_run(
        &self,
        input: NewCatalogSyncRun,
    ) -> Result<CatalogSyncRunSnapshot, CatalogSyncError> {
        let source = get_source_model(&self.database, input.source_id)
            .await?
            .ok_or(CatalogSyncError::SourceNotFound {
                source_id: input.source_id,
            })?;
        if !source.enabled {
            return Err(CatalogSyncError::SourceDisabled {
                source_id: input.source_id,
            });
        }
        let run_id = input.run_id.unwrap_or_else(Uuid::new_v4);
        let active_model = catalog_sync_run::ActiveModel {
            id: NotSet,
            run_id: Set(run_id),
            source_db_id: Set(source.id),
            status: Set(SyncRunStatus::Queued.as_str().to_owned()),
            requested_cursor: Set(input.requested_cursor),
            result_cursor: Set(None),
            observed_count: Set(0),
            error_message: Set(None),
            row_version: Set(1),
            created_at: NotSet,
            started_at: Set(None),
            finished_at: Set(None),
        };
        if let Err(error) = catalog_sync_run::Entity::insert(active_model)
            .exec_without_returning(&self.database)
            .await
        {
            if get_run_model(&self.database, run_id).await?.is_some() {
                return Err(CatalogSyncError::RunAlreadyExists { run_id });
            }
            return Err(error.into());
        }
        self.get_run(run_id)
            .await?
            .ok_or(CatalogSyncError::RunNotFound { run_id })
    }

    pub async fn get_run(
        &self,
        run_id: Uuid,
    ) -> Result<Option<CatalogSyncRunSnapshot>, CatalogSyncError> {
        let Some(run) = get_run_model(&self.database, run_id).await? else {
            return Ok(None);
        };
        let source = catalog_source::Entity::find_by_id(run.source_db_id)
            .one(&self.database)
            .await?
            .ok_or_else(|| missing_run_source(run_id))?;
        run_model_to_snapshot(run, source.source_id).map(Some)
    }

    /// Atomically changes a queued run to running. The row-version predicate
    /// makes competing worker claims deterministic: only one claim succeeds.
    pub async fn claim_run(
        &self,
        run_id: Uuid,
        expected: CatalogRowVersion,
    ) -> Result<CatalogSyncRunSnapshot, CatalogSyncError> {
        let transaction = self.database.begin().await?;
        let run = get_run_model(&transaction, run_id)
            .await?
            .ok_or(CatalogSyncError::RunNotFound { run_id })?;
        let actual = parse_row_version("sync run", run_id, run.row_version)?;
        if actual != expected {
            return Err(CatalogSyncError::RunConflict {
                run_id,
                expected,
                actual,
            });
        }
        let status = parse_run_status(run_id, &run.status)?;
        if status != SyncRunStatus::Queued {
            return Err(CatalogSyncError::InvalidRunTransition {
                run_id,
                from: status,
                to: SyncRunStatus::Running,
            });
        }
        let source = catalog_source::Entity::find_by_id(run.source_db_id)
            .one(&transaction)
            .await?
            .ok_or_else(|| missing_run_source(run_id))?;
        if !source.enabled {
            return Err(CatalogSyncError::SourceDisabled {
                source_id: source.source_id,
            });
        }

        // `last_seen_at >= started_at` is used to make unchanged observations
        // idempotent within a run. That inference is only sound when runs for
        // one source cannot overlap, so the source row is also advanced under
        // CAS as a durable claim mutex.
        let another_running = catalog_sync_run::Entity::find()
            .filter(catalog_sync_run::Column::SourceDbId.eq(source.id))
            .filter(catalog_sync_run::Column::RunId.ne(run_id))
            .filter(catalog_sync_run::Column::Status.eq(SyncRunStatus::Running.as_str()))
            .one(&transaction)
            .await?
            .is_some();
        if another_running {
            return Err(CatalogSyncError::SourceBusy {
                source_id: source.source_id,
            });
        }
        let source_version =
            parse_row_version("catalog source", source.source_id, source.row_version)?;
        let next_source_version =
            next_row_version("catalog source", source.source_id, source_version)?;
        let source_claim = catalog_source::Entity::update_many()
            .col_expr(
                catalog_source::Column::RowVersion,
                Expr::value(row_version_as_i64(
                    "catalog source",
                    source.source_id,
                    next_source_version,
                )?),
            )
            .col_expr(
                catalog_source::Column::UpdatedAt,
                Expr::current_timestamp().into(),
            )
            .filter(catalog_source::Column::Id.eq(source.id))
            .filter(catalog_source::Column::RowVersion.eq(row_version_as_i64(
                "catalog source",
                source.source_id,
                source_version,
            )?))
            .exec(&transaction)
            .await?;
        if source_claim.rows_affected != 1 {
            return Err(CatalogSyncError::SourceBusy {
                source_id: source.source_id,
            });
        }

        let next = next_row_version("sync run", run_id, expected)?;
        let result = catalog_sync_run::Entity::update_many()
            .col_expr(
                catalog_sync_run::Column::Status,
                Expr::value(SyncRunStatus::Running.as_str()),
            )
            .col_expr(catalog_sync_run::Column::StartedAt, Expr::value(now()))
            .col_expr(
                catalog_sync_run::Column::RowVersion,
                Expr::value(row_version_as_i64("sync run", run_id, next)?),
            )
            .filter(catalog_sync_run::Column::RunId.eq(run_id))
            .filter(
                catalog_sync_run::Column::RowVersion
                    .eq(row_version_as_i64("sync run", run_id, expected)?),
            )
            .filter(catalog_sync_run::Column::Status.eq(SyncRunStatus::Queued.as_str()))
            .exec(&transaction)
            .await?;
        if result.rows_affected != 1 {
            return run_conflict_or_not_found(&transaction, run_id, expected).await;
        }
        let updated = get_run_model(&transaction, run_id)
            .await?
            .ok_or(CatalogSyncError::RunNotFound { run_id })?;
        let snapshot = run_model_to_snapshot(updated, source.source_id)?;
        transaction.commit().await?;
        Ok(snapshot)
    }

    /// Persist an observation and advance the run under one transaction.
    ///
    /// `expected` is the run version returned by `claim_run` or the previous
    /// call. This serializes a worker's progress and makes retry behavior
    /// explicit to both Web clients and background jobs.
    pub async fn record_observation(
        &self,
        run_id: Uuid,
        expected: CatalogRowVersion,
        input: CatalogReleaseObservation,
    ) -> Result<RecordedCatalogObservation, CatalogSyncError> {
        validate_observation(&input)?;
        let raw_sha256 = sha256(input.raw_document.as_bytes());
        let transaction = self.database.begin().await?;
        let run = get_run_model(&transaction, run_id)
            .await?
            .ok_or(CatalogSyncError::RunNotFound { run_id })?;
        let actual = parse_row_version("sync run", run_id, run.row_version)?;
        if actual != expected {
            return Err(CatalogSyncError::RunConflict {
                run_id,
                expected,
                actual,
            });
        }
        let status = parse_run_status(run_id, &run.status)?;
        if status != SyncRunStatus::Running {
            return Err(CatalogSyncError::RunNotRunning { run_id, status });
        }
        let started_at = run
            .started_at
            .ok_or_else(|| CatalogSyncError::InvalidPersistedValue {
                entity: "sync run",
                id: run_id,
                field: "started_at",
                value: "missing for running run".to_owned(),
            })?;
        let source = catalog_source::Entity::find_by_id(run.source_db_id)
            .one(&transaction)
            .await?
            .ok_or_else(|| missing_run_source(run_id))?;
        let observed_at = now();

        let existing =
            get_source_release_model(&transaction, run.source_db_id, &input.external_release_id)
                .await?;

        let (release, revision, revision_appended, first_observation_in_run) =
            if let Some(existing) = existing {
                let latest = latest_revision(&transaction, existing.id).await?.ok_or(
                    CatalogSyncError::MissingObservationRevision {
                        source_release_id: existing.source_release_id,
                    },
                )?;
                let latest_revision = checked_revision(&latest, existing.source_release_id)?;
                let latest_digest = checked_revision_digest(&latest, existing.source_release_id)?;
                let same_content = latest_digest == raw_sha256
                    && latest.raw_document == input.raw_document
                    && latest.parsed_document == input.parsed_document;
                let seen_in_this_run = existing.last_seen_at >= started_at;

                // A worker retry after a timeout is a read, not a new event.
                // The expected run version is still checked above, so stale
                // callers cannot mistake another worker's progress for theirs.
                if seen_in_this_run
                    && same_content
                    && existing.source_url == input.source_url
                    && existing.not_seen_since.is_none()
                {
                    let run_snapshot = run_model_to_snapshot(run, source.source_id)?;
                    let observation =
                        observation_snapshot(existing, latest_revision, latest_digest)?;
                    transaction.commit().await?;
                    return Ok(RecordedCatalogObservation {
                        run: run_snapshot,
                        observation,
                        revision_appended: false,
                        first_observation_in_run: false,
                    });
                }

                let expected_release_version = parse_row_version(
                    "source release",
                    existing.source_release_id,
                    existing.row_version,
                )?;
                let next_release_version = next_row_version(
                    "source release",
                    existing.source_release_id,
                    expected_release_version,
                )?;
                let result = catalog_source_release::Entity::update_many()
                    .col_expr(
                        catalog_source_release::Column::SourceUrl,
                        Expr::value(&input.source_url),
                    )
                    .col_expr(
                        catalog_source_release::Column::LastSeenAt,
                        Expr::value(observed_at),
                    )
                    .col_expr(
                        catalog_source_release::Column::NotSeenSince,
                        Expr::value(None::<DateTimeUtc>),
                    )
                    .col_expr(
                        catalog_source_release::Column::RowVersion,
                        Expr::value(row_version_as_i64(
                            "source release",
                            existing.source_release_id,
                            next_release_version,
                        )?),
                    )
                    .filter(catalog_source_release::Column::Id.eq(existing.id))
                    .filter(
                        catalog_source_release::Column::RowVersion.eq(row_version_as_i64(
                            "source release",
                            existing.source_release_id,
                            expected_release_version,
                        )?),
                    )
                    .exec(&transaction)
                    .await?;
                if result.rows_affected != 1 {
                    return Err(CatalogSyncError::ObservationConflict {
                        source_release_id: existing.source_release_id,
                    });
                }

                let revision = if same_content {
                    latest_revision
                } else {
                    let next_revision = next_revision(existing.source_release_id, latest_revision)?;
                    insert_revision(
                        &transaction,
                        existing.id,
                        next_revision,
                        run_id,
                        &input,
                        raw_sha256,
                        observed_at,
                    )
                    .await?;
                    next_revision
                };
                let release = catalog_source_release::Entity::find_by_id(existing.id)
                    .one(&transaction)
                    .await?
                    .ok_or(CatalogSyncError::ObservationConflict {
                        source_release_id: existing.source_release_id,
                    })?;
                (release, revision, !same_content, !seen_in_this_run)
            } else {
                let source_release_id = Uuid::new_v4();
                let insert =
                    catalog_source_release::Entity::insert(catalog_source_release::ActiveModel {
                        id: NotSet,
                        source_release_id: Set(source_release_id),
                        source_db_id: Set(run.source_db_id),
                        external_release_id: Set(input.external_release_id.clone()),
                        source_url: Set(input.source_url.clone()),
                        linked_release_db_id: Set(None),
                        first_seen_at: Set(observed_at),
                        last_seen_at: Set(observed_at),
                        not_seen_since: Set(None),
                        row_version: Set(1),
                    })
                    .exec(&transaction)
                    .await?;
                insert_revision(
                    &transaction,
                    insert.last_insert_id,
                    1,
                    run_id,
                    &input,
                    raw_sha256,
                    observed_at,
                )
                .await?;
                let release = catalog_source_release::Entity::find_by_id(insert.last_insert_id)
                    .one(&transaction)
                    .await?
                    .ok_or(CatalogSyncError::ObservationConflict { source_release_id })?;
                (release, 1, true, true)
            };

        let next_run_version = next_row_version("sync run", run_id, expected)?;
        let observed_count = checked_observed_count(run_id, run.observed_count)?;
        let next_observed_count = if first_observation_in_run {
            observed_count
                .checked_add(1)
                .filter(|value| i32::try_from(*value).is_ok())
                .ok_or(CatalogSyncError::NumericOutOfRange {
                    entity: "sync run",
                    id: run_id,
                    field: "observed_count",
                })?
        } else {
            observed_count
        };
        let result = catalog_sync_run::Entity::update_many()
            .col_expr(
                catalog_sync_run::Column::ObservedCount,
                Expr::value(i32::try_from(next_observed_count).map_err(|_| {
                    CatalogSyncError::NumericOutOfRange {
                        entity: "sync run",
                        id: run_id,
                        field: "observed_count",
                    }
                })?),
            )
            .col_expr(
                catalog_sync_run::Column::RowVersion,
                Expr::value(row_version_as_i64("sync run", run_id, next_run_version)?),
            )
            .filter(catalog_sync_run::Column::RunId.eq(run_id))
            .filter(
                catalog_sync_run::Column::RowVersion
                    .eq(row_version_as_i64("sync run", run_id, expected)?),
            )
            .filter(catalog_sync_run::Column::Status.eq(SyncRunStatus::Running.as_str()))
            .exec(&transaction)
            .await?;
        if result.rows_affected != 1 {
            return run_conflict_or_not_found(&transaction, run_id, expected).await;
        }

        let updated_run = get_run_model(&transaction, run_id)
            .await?
            .ok_or(CatalogSyncError::RunNotFound { run_id })?;
        let run_snapshot = run_model_to_snapshot(updated_run, source.source_id)?;
        let observation = observation_snapshot(release, revision, raw_sha256)?;
        transaction.commit().await?;
        Ok(RecordedCatalogObservation {
            run: run_snapshot,
            observation,
            revision_appended,
            first_observation_in_run,
        })
    }

    /// Finish a run. Only a successful run is allowed to infer absence; failed
    /// or cancelled runs leave the previous collection view untouched.
    pub async fn finish_run(
        &self,
        run_id: Uuid,
        expected: CatalogRowVersion,
        outcome: FinishCatalogSyncRun,
    ) -> Result<FinishedCatalogSyncRun, CatalogSyncError> {
        if matches!(&outcome, FinishCatalogSyncRun::Failed { error_message } if error_message.is_empty())
        {
            return Err(CatalogSyncError::InvalidInput {
                field: "error_message",
                message: "failure must include an error message",
            });
        }
        let transaction = self.database.begin().await?;
        let run = get_run_model(&transaction, run_id)
            .await?
            .ok_or(CatalogSyncError::RunNotFound { run_id })?;
        let actual = parse_row_version("sync run", run_id, run.row_version)?;
        if actual != expected {
            return Err(CatalogSyncError::RunConflict {
                run_id,
                expected,
                actual,
            });
        }
        let status = parse_run_status(run_id, &run.status)?;
        let next_status = outcome.status();
        let stored_error = match &outcome {
            FinishCatalogSyncRun::Succeeded { .. } => None,
            FinishCatalogSyncRun::Failed { error_message } => Some(
                sanitize_sync_error(error_message)
                    .unwrap_or_else(|| "source adapter failed".to_owned()),
            ),
            FinishCatalogSyncRun::Cancelled { message } => {
                message.as_deref().and_then(sanitize_sync_error)
            }
        };
        let source = catalog_source::Entity::find_by_id(run.source_db_id)
            .one(&transaction)
            .await?
            .ok_or_else(|| missing_run_source(run_id))?;

        // Retrying a completed command with the current version is a no-op.
        if status == next_status
            && run.result_cursor.as_deref() == outcome.result_cursor()
            && run.error_message.as_deref() == stored_error.as_deref()
            && run.finished_at.is_some()
        {
            let snapshot = run_model_to_snapshot(run, source.source_id)?;
            transaction.commit().await?;
            return Ok(FinishedCatalogSyncRun {
                run: snapshot,
                newly_not_seen: 0,
            });
        }
        if !status.can_transition_to(next_status) || status == next_status {
            return Err(CatalogSyncError::InvalidRunTransition {
                run_id,
                from: status,
                to: next_status,
            });
        }

        let finished_at = now();
        let mut newly_not_seen = 0_u32;
        if next_status == SyncRunStatus::Succeeded {
            let started_at =
                run.started_at
                    .ok_or_else(|| CatalogSyncError::InvalidPersistedValue {
                        entity: "sync run",
                        id: run_id,
                        field: "started_at",
                        value: "missing for successful run".to_owned(),
                    })?;
            let absent = catalog_source_release::Entity::find()
                .filter(catalog_source_release::Column::SourceDbId.eq(run.source_db_id))
                .filter(catalog_source_release::Column::LastSeenAt.lt(started_at))
                .filter(catalog_source_release::Column::NotSeenSince.is_null())
                .all(&transaction)
                .await?;
            for release in absent {
                let current = parse_row_version(
                    "source release",
                    release.source_release_id,
                    release.row_version,
                )?;
                let next = next_row_version("source release", release.source_release_id, current)?;
                let result = catalog_source_release::Entity::update_many()
                    .col_expr(
                        catalog_source_release::Column::NotSeenSince,
                        Expr::value(finished_at),
                    )
                    .col_expr(
                        catalog_source_release::Column::RowVersion,
                        Expr::value(row_version_as_i64(
                            "source release",
                            release.source_release_id,
                            next,
                        )?),
                    )
                    .filter(catalog_source_release::Column::Id.eq(release.id))
                    .filter(
                        catalog_source_release::Column::RowVersion.eq(row_version_as_i64(
                            "source release",
                            release.source_release_id,
                            current,
                        )?),
                    )
                    .exec(&transaction)
                    .await?;
                if result.rows_affected != 1 {
                    return Err(CatalogSyncError::ObservationConflict {
                        source_release_id: release.source_release_id,
                    });
                }
                newly_not_seen =
                    newly_not_seen
                        .checked_add(1)
                        .ok_or(CatalogSyncError::NumericOutOfRange {
                            entity: "sync run",
                            id: run_id,
                            field: "newly_not_seen",
                        })?;
            }
        }

        let next = next_row_version("sync run", run_id, expected)?;
        let result = catalog_sync_run::Entity::update_many()
            .col_expr(
                catalog_sync_run::Column::Status,
                Expr::value(next_status.as_str()),
            )
            .col_expr(
                catalog_sync_run::Column::ResultCursor,
                Expr::value(outcome.result_cursor().map(str::to_owned)),
            )
            .col_expr(
                catalog_sync_run::Column::ErrorMessage,
                Expr::value(stored_error),
            )
            .col_expr(
                catalog_sync_run::Column::FinishedAt,
                Expr::value(finished_at),
            )
            .col_expr(
                catalog_sync_run::Column::RowVersion,
                Expr::value(row_version_as_i64("sync run", run_id, next)?),
            )
            .filter(catalog_sync_run::Column::RunId.eq(run_id))
            .filter(
                catalog_sync_run::Column::RowVersion
                    .eq(row_version_as_i64("sync run", run_id, expected)?),
            )
            .filter(catalog_sync_run::Column::Status.eq(status.as_str()))
            .exec(&transaction)
            .await?;
        if result.rows_affected != 1 {
            return run_conflict_or_not_found(&transaction, run_id, expected).await;
        }

        let updated = get_run_model(&transaction, run_id)
            .await?
            .ok_or(CatalogSyncError::RunNotFound { run_id })?;
        let snapshot = run_model_to_snapshot(updated, source.source_id)?;
        transaction.commit().await?;
        Ok(FinishedCatalogSyncRun {
            run: snapshot,
            newly_not_seen,
        })
    }
}

async fn get_artist_model<C: ConnectionTrait>(
    connection: &C,
    artist_id: Uuid,
) -> Result<Option<catalog_artist::Model>, DbErr> {
    catalog_artist::Entity::find()
        .filter(catalog_artist::Column::ArtistId.eq(artist_id))
        .one(connection)
        .await
}

async fn get_source_model<C: ConnectionTrait>(
    connection: &C,
    source_id: Uuid,
) -> Result<Option<catalog_source::Model>, DbErr> {
    catalog_source::Entity::find()
        .filter(catalog_source::Column::SourceId.eq(source_id))
        .one(connection)
        .await
}

async fn get_run_model<C: ConnectionTrait>(
    connection: &C,
    run_id: Uuid,
) -> Result<Option<catalog_sync_run::Model>, DbErr> {
    catalog_sync_run::Entity::find()
        .filter(catalog_sync_run::Column::RunId.eq(run_id))
        .one(connection)
        .await
}

async fn get_source_release_model<C: ConnectionTrait>(
    connection: &C,
    source_db_id: i32,
    external_release_id: &str,
) -> Result<Option<catalog_source_release::Model>, DbErr> {
    catalog_source_release::Entity::find()
        .filter(catalog_source_release::Column::SourceDbId.eq(source_db_id))
        .filter(catalog_source_release::Column::ExternalReleaseId.eq(external_release_id))
        .one(connection)
        .await
}

async fn latest_revision<C: ConnectionTrait>(
    connection: &C,
    source_release_db_id: i32,
) -> Result<Option<catalog_source_release_revision::Model>, DbErr> {
    catalog_source_release_revision::Entity::find()
        .filter(catalog_source_release_revision::Column::SourceReleaseDbId.eq(source_release_db_id))
        .order_by_desc(catalog_source_release_revision::Column::Revision)
        .one(connection)
        .await
}

#[allow(clippy::too_many_arguments)]
async fn insert_revision<C: ConnectionTrait>(
    connection: &C,
    source_release_db_id: i32,
    revision: u64,
    run_id: Uuid,
    input: &CatalogReleaseObservation,
    raw_sha256: Digest,
    observed_at: PersistedTimestamp,
) -> Result<(), CatalogSyncError> {
    let persisted_revision =
        i64::try_from(revision).map_err(|_| CatalogSyncError::NumericOutOfRange {
            entity: "source release revision",
            id: run_id,
            field: "revision",
        })?;
    catalog_source_release_revision::Entity::insert(catalog_source_release_revision::ActiveModel {
        id: NotSet,
        source_release_db_id: Set(source_release_db_id),
        revision: Set(persisted_revision),
        sync_run_id: Set(run_id),
        raw_document: Set(input.raw_document.clone()),
        parsed_document: Set(input.parsed_document.clone()),
        raw_sha256: Set(raw_sha256.as_bytes().to_vec()),
        observed_at: Set(observed_at),
    })
    .exec_without_returning(connection)
    .await?;
    Ok(())
}

async fn run_conflict_or_not_found<C: ConnectionTrait, T>(
    connection: &C,
    run_id: Uuid,
    expected: CatalogRowVersion,
) -> Result<T, CatalogSyncError> {
    match get_run_model(connection, run_id).await? {
        Some(actual) => Err(CatalogSyncError::RunConflict {
            run_id,
            expected,
            actual: parse_row_version("sync run", run_id, actual.row_version)?,
        }),
        None => Err(CatalogSyncError::RunNotFound { run_id }),
    }
}

fn source_model_to_snapshot(
    model: catalog_source::Model,
    artist_id: Uuid,
) -> Result<CatalogSourceSnapshot, CatalogSyncError> {
    let source_id = model.source_id;
    let kind = CatalogSourceKind::from_str(&model.kind).map_err(|_| {
        CatalogSyncError::InvalidPersistedValue {
            entity: "catalog source",
            id: source_id,
            field: "kind",
            value: model.kind.clone(),
        }
    })?;
    Ok(CatalogSourceSnapshot {
        source_id,
        artist_id,
        kind,
        locator: model.locator,
        storefront: model.storefront,
        locale: model.locale,
        enabled: model.enabled,
        row_version: parse_row_version("catalog source", source_id, model.row_version)?,
        created_at: timestamp(model.created_at),
        updated_at: timestamp(model.updated_at),
    })
}

fn run_model_to_snapshot(
    model: catalog_sync_run::Model,
    source_id: Uuid,
) -> Result<CatalogSyncRunSnapshot, CatalogSyncError> {
    let run_id = model.run_id;
    Ok(CatalogSyncRunSnapshot {
        run_id,
        source_id,
        status: parse_run_status(run_id, &model.status)?,
        requested_cursor: model.requested_cursor,
        result_cursor: model.result_cursor,
        observed_count: checked_observed_count(run_id, model.observed_count)?,
        error_message: model.error_message,
        row_version: parse_row_version("sync run", run_id, model.row_version)?,
        created_at: timestamp(model.created_at),
        started_at: model.started_at.map(timestamp),
        finished_at: model.finished_at.map(timestamp),
    })
}

fn observation_snapshot(
    model: catalog_source_release::Model,
    revision: u64,
    raw_sha256: Digest,
) -> Result<CatalogObservationSnapshot, CatalogSyncError> {
    let source_release_id = model.source_release_id;
    Ok(CatalogObservationSnapshot {
        source_release_id,
        external_release_id: model.external_release_id,
        source_url: model.source_url,
        revision,
        raw_sha256,
        first_seen_at: timestamp(model.first_seen_at),
        last_seen_at: timestamp(model.last_seen_at),
        not_seen_since: model.not_seen_since.map(timestamp),
        row_version: parse_row_version("source release", source_release_id, model.row_version)?,
    })
}

fn checked_revision(
    model: &catalog_source_release_revision::Model,
    source_release_id: Uuid,
) -> Result<u64, CatalogSyncError> {
    u64::try_from(model.revision)
        .ok()
        .filter(|revision| *revision > 0)
        .ok_or_else(|| CatalogSyncError::InvalidPersistedValue {
            entity: "source release",
            id: source_release_id,
            field: "revision",
            value: model.revision.to_string(),
        })
}

fn checked_revision_digest(
    model: &catalog_source_release_revision::Model,
    source_release_id: Uuid,
) -> Result<Digest, CatalogSyncError> {
    let revision = checked_revision(model, source_release_id)?;
    let actual = model.raw_sha256.len();
    let stored = <[u8; Digest::LENGTH]>::try_from(model.raw_sha256.as_slice())
        .map(Digest::new)
        .map_err(|_| CatalogSyncError::InvalidObservationDigestLength {
            source_release_id,
            revision,
            actual,
        })?;
    if sha256(model.raw_document.as_bytes()) != stored {
        return Err(CatalogSyncError::ObservationDigestMismatch {
            source_release_id,
            revision,
        });
    }
    Ok(stored)
}

fn validate_observation(input: &CatalogReleaseObservation) -> Result<(), CatalogSyncError> {
    require_non_empty("external_release_id", &input.external_release_id)?;
    require_non_empty("source_url", &input.source_url)?;
    require_non_empty("raw_document", &input.raw_document)?;
    require_non_empty("parsed_document", &input.parsed_document)
}

fn require_non_empty(field: &'static str, value: &str) -> Result<(), CatalogSyncError> {
    if value.is_empty() {
        Err(CatalogSyncError::InvalidInput {
            field,
            message: "value must not be empty",
        })
    } else {
        Ok(())
    }
}

/// Keep operator-facing failures useful without persisting credentials or
/// signed endpoints returned by a source adapter. Detailed adapter errors
/// belong in a worker-local, access-controlled log.
fn sanitize_sync_error(message: &str) -> Option<String> {
    let lowercase = message.to_ascii_lowercase();
    let sensitive = message.contains("://")
        || message.contains('?')
        || [
            "authorization",
            "credential",
            "password",
            "secret",
            "signature",
            "token",
        ]
        .iter()
        .any(|needle| lowercase.contains(needle));
    if sensitive {
        return None;
    }

    let sanitized: String = message
        .chars()
        .filter(|character| !character.is_control())
        .take(MAX_SYNC_ERROR_CHARS)
        .collect();
    (!sanitized.is_empty()).then_some(sanitized)
}

fn parse_run_status(run_id: Uuid, value: &str) -> Result<SyncRunStatus, CatalogSyncError> {
    SyncRunStatus::from_str(value).map_err(|_| CatalogSyncError::InvalidPersistedValue {
        entity: "sync run",
        id: run_id,
        field: "status",
        value: value.to_owned(),
    })
}

fn checked_observed_count(run_id: Uuid, value: i32) -> Result<u32, CatalogSyncError> {
    u32::try_from(value).map_err(|_| CatalogSyncError::InvalidPersistedValue {
        entity: "sync run",
        id: run_id,
        field: "observed_count",
        value: value.to_string(),
    })
}

fn parse_row_version(
    entity: &'static str,
    id: Uuid,
    value: i64,
) -> Result<CatalogRowVersion, CatalogSyncError> {
    u64::try_from(value)
        .ok()
        .and_then(CatalogRowVersion::new)
        .ok_or_else(|| CatalogSyncError::InvalidPersistedValue {
            entity,
            id,
            field: "row_version",
            value: value.to_string(),
        })
}

fn row_version_as_i64(
    entity: &'static str,
    id: Uuid,
    version: CatalogRowVersion,
) -> Result<i64, CatalogSyncError> {
    i64::try_from(version.get()).map_err(|_| CatalogSyncError::NumericOutOfRange {
        entity,
        id,
        field: "row_version",
    })
}

fn next_row_version(
    entity: &'static str,
    id: Uuid,
    version: CatalogRowVersion,
) -> Result<CatalogRowVersion, CatalogSyncError> {
    version
        .get()
        .checked_add(1)
        .filter(|value| i64::try_from(*value).is_ok())
        .and_then(CatalogRowVersion::new)
        .ok_or(CatalogSyncError::NumericOutOfRange {
            entity,
            id,
            field: "row_version",
        })
}

fn next_revision(source_release_id: Uuid, revision: u64) -> Result<u64, CatalogSyncError> {
    revision
        .checked_add(1)
        .filter(|value| i64::try_from(*value).is_ok())
        .ok_or(CatalogSyncError::NumericOutOfRange {
            entity: "source release",
            id: source_release_id,
            field: "revision",
        })
}

fn sha256(value: &[u8]) -> Digest {
    let mut hasher = Sha256::new();
    hasher.update(value);
    Digest::new(hasher.finalize().into())
}

fn missing_run_source(run_id: Uuid) -> CatalogSyncError {
    CatalogSyncError::Database(DbErr::Custom(format!(
        "catalog sync run {run_id} references a missing source"
    )))
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use sea_orm::{ConnectOptions, Database};
    use sea_orm_migration::MigratorTrait;

    use super::*;
    use crate::migrator::Migrator;

    async fn migrated_database() -> DatabaseConnection {
        let mut options = ConnectOptions::new("sqlite::memory:");
        options.max_connections(1);
        let database = Database::connect(options).await.unwrap();
        Migrator::up(&database, None).await.unwrap();
        database
    }

    async fn insert_artist(database: &DatabaseConnection, artist_id: Uuid) {
        catalog_artist::Entity::insert(catalog_artist::ActiveModel {
            id: NotSet,
            artist_id: Set(artist_id),
            display_name: Set("Artist（公式）".to_owned()),
            sort_name: Set(None),
            notes: Set(None),
            row_version: Set(1),
            created_at: NotSet,
            updated_at: NotSet,
        })
        .exec_without_returning(database)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn observations_are_idempotent_versioned_and_never_deleted_when_absent() {
        let database = migrated_database().await;
        let artist_id = Uuid::new_v4();
        insert_artist(&database, artist_id).await;
        let service = CatalogSyncService::new(database.clone());

        let source = service
            .create_source(NewCatalogSource {
                source_id: None,
                artist_id,
                kind: CatalogSourceKind::Vgmdb,
                locator: "https://vgmdb.net/artist/1234".to_owned(),
                storefront: None,
                locale: Some("ja-JP".to_owned()),
                configuration_document: Some(r#"{"page_size":50}"#.to_owned()),
                secret_ref: Some("secret/catalog/vgmdb".to_owned()),
            })
            .await
            .unwrap();
        let listed = service.list_sources_for_artist(artist_id).await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].source_id, source.source_id);
        assert_eq!(listed[0].locator, "https://vgmdb.net/artist/1234");

        let first = CatalogReleaseObservation {
            external_release_id: "album/作品・A〜B～C".to_owned(),
            source_url: "https://vgmdb.net/album/100".to_owned(),
            raw_document: r#"{"title":"作品・A〜B～C (初回)"}"#.to_owned(),
            parsed_document: r#"{"title":"作品・A〜B～C (初回)","kind":"album"}"#.to_owned(),
        };
        let second = CatalogReleaseObservation {
            external_release_id: "album/別作品".to_owned(),
            source_url: "https://vgmdb.net/album/200".to_owned(),
            raw_document: r#"{"title":"別作品"}"#.to_owned(),
            parsed_document: r#"{"title":"別作品","kind":"album"}"#.to_owned(),
        };

        let queued = service
            .start_run(NewCatalogSyncRun {
                run_id: None,
                source_id: source.source_id,
                requested_cursor: None,
            })
            .await
            .unwrap();
        let running = service
            .claim_run(queued.run_id, queued.row_version)
            .await
            .unwrap();
        let competing = service
            .start_run(NewCatalogSyncRun {
                run_id: None,
                source_id: source.source_id,
                requested_cursor: None,
            })
            .await
            .unwrap();
        assert!(matches!(
            service
                .claim_run(competing.run_id, competing.row_version)
                .await,
            Err(CatalogSyncError::SourceBusy { .. })
        ));
        let recorded = service
            .record_observation(running.run_id, running.row_version, first.clone())
            .await
            .unwrap();
        assert_eq!(recorded.run.observed_count, 1);
        assert_eq!(recorded.observation.revision, 1);
        assert!(recorded.revision_appended);
        assert!(recorded.first_observation_in_run);

        let replayed = service
            .record_observation(recorded.run.run_id, recorded.run.row_version, first.clone())
            .await
            .unwrap();
        assert_eq!(replayed.run.row_version, recorded.run.row_version);
        assert_eq!(replayed.run.observed_count, 1);
        assert!(!replayed.revision_appended);
        assert!(!replayed.first_observation_in_run);

        let recorded_second = service
            .record_observation(
                replayed.run.run_id,
                replayed.run.row_version,
                second.clone(),
            )
            .await
            .unwrap();
        assert_eq!(recorded_second.run.observed_count, 2);
        let finished = service
            .finish_run(
                recorded_second.run.run_id,
                recorded_second.run.row_version,
                FinishCatalogSyncRun::Succeeded {
                    result_cursor: Some("page:2".to_owned()),
                },
            )
            .await
            .unwrap();
        assert_eq!(finished.newly_not_seen, 0);

        // A later successful run sees only the first release. Its unchanged
        // response is not copied into a new revision, while the second release
        // is retained and receives its first absence timestamp.
        let queued = service
            .start_run(NewCatalogSyncRun {
                run_id: None,
                source_id: source.source_id,
                requested_cursor: finished.run.result_cursor.clone(),
            })
            .await
            .unwrap();
        let running = service
            .claim_run(queued.run_id, queued.row_version)
            .await
            .unwrap();
        let recorded = service
            .record_observation(running.run_id, running.row_version, first.clone())
            .await
            .unwrap();
        assert!(!recorded.revision_appended);
        assert!(recorded.first_observation_in_run);
        let finished = service
            .finish_run(
                recorded.run.run_id,
                recorded.run.row_version,
                FinishCatalogSyncRun::Succeeded {
                    result_cursor: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(finished.newly_not_seen, 1);
        let absent = get_source_release_model(
            &database,
            catalog_source::Entity::find()
                .filter(catalog_source::Column::SourceId.eq(source.source_id))
                .one(&database)
                .await
                .unwrap()
                .unwrap()
                .id,
            &second.external_release_id,
        )
        .await
        .unwrap()
        .unwrap();
        assert!(absent.not_seen_since.is_some());

        // Re-parsing the same raw document differently is new reviewable
        // evidence even though its raw SHA-256 is unchanged.
        let queued = service
            .start_run(NewCatalogSyncRun {
                run_id: None,
                source_id: source.source_id,
                requested_cursor: None,
            })
            .await
            .unwrap();
        let running = service
            .claim_run(queued.run_id, queued.row_version)
            .await
            .unwrap();
        let mut reparsed = first;
        reparsed.parsed_document =
            r#"{"title":"作品・A〜B～C (初回)","kind":"album","parser":2}"#.to_owned();
        let recorded = service
            .record_observation(running.run_id, running.row_version, reparsed)
            .await
            .unwrap();
        assert!(recorded.revision_appended);
        assert_eq!(recorded.observation.revision, 2);
    }

    #[test]
    fn commands_and_failures_do_not_expose_source_secrets() {
        let source = NewCatalogSource {
            source_id: None,
            artist_id: Uuid::new_v4(),
            kind: CatalogSourceKind::ArtistWebsite,
            locator: "https://artist.example/discography?token=visible".to_owned(),
            storefront: None,
            locale: None,
            configuration_document: Some("{\"authorization\":\"secret-value\"}".to_owned()),
            secret_ref: Some("secret/catalog/artist".to_owned()),
        };
        let debug = format!("{source:?}");
        assert!(!debug.contains("artist.example"));
        assert!(!debug.contains("secret-value"));
        assert!(!debug.contains("secret/catalog/artist"));

        assert_eq!(
            sanitize_sync_error("upstream returned 503"),
            Some("upstream returned 503".to_owned())
        );
        assert_eq!(
            sanitize_sync_error("GET https://artist.example/a?token=secret failed"),
            None
        );
        assert_eq!(sanitize_sync_error("token expired"), None);
        assert_eq!(
            sanitize_sync_error(&"x".repeat(MAX_SYNC_ERROR_CHARS + 20))
                .expect("plain message is retained")
                .chars()
                .count(),
            MAX_SYNC_ERROR_CHARS
        );
    }
}
