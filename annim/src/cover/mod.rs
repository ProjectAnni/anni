//! Durable cover discovery, download leasing, and selection.
//!
//! Remote URLs are deliberately confined to persistence and [`CoverFetchLease`].
//! Ordinary API snapshots reveal only whether a remote URL exists, so signed
//! query parameters cannot accidentally escape through GraphQL or logs.

use std::{num::NonZeroU32, str::FromStr, time::Duration};

use anni_catalog::{
    canonicalize_cover_url, cover_asset_storage_key, preferred_amazon_artwork_url,
    preferred_apple_artwork_url, CoverCandidateState, CoverMediaType, CoverSourceKind,
    CoverUrlError,
};
use anni_ingest::Digest;
use sea_orm::{
    prelude::{DateTimeUtc, Uuid},
    sea_query::{Condition, Expr, OnConflict},
    ActiveValue::{NotSet, Set},
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, QueryOrder,
    TransactionTrait, TryInsertResult,
};
use thiserror::Error;

use crate::entities::{
    catalog_release, cover_asset, cover_candidate, cover_selection,
    helper::{now, timestamp},
};

const MAX_CLAIM_CAS_ATTEMPTS: usize = 16;
const MAX_FAILURE_CODE_BYTES: usize = 64;
const MAX_FAILURE_MESSAGE_CHARS: usize = 512;

/// Optimistic-lock version shared by candidates and selections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CoverRowVersion(u64);

impl CoverRowVersion {
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

    fn as_i64(self, entity: &'static str, id: Uuid) -> Result<i64, CoverError> {
        i64::try_from(self.0).map_err(|_| CoverError::NumericOutOfRange {
            entity,
            id,
            field: "row_version",
        })
    }

    fn next(self, entity: &'static str, id: Uuid) -> Result<Self, CoverError> {
        self.0
            .checked_add(1)
            .filter(|value| i64::try_from(*value).is_ok())
            .map(Self)
            .ok_or(CoverError::NumericOutOfRange {
                entity,
                id,
                field: "row_version",
            })
    }
}

impl std::fmt::Display for CoverRowVersion {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverAssetSnapshot {
    pub asset_id: Uuid,
    pub content_sha256: Digest,
    pub storage_key: String,
    pub media_type: CoverMediaType,
    pub width: NonZeroU32,
    pub height: NonZeroU32,
    pub byte_length: u64,
    pub fetched_at: DateTimeUtc,
    pub verified_at: DateTimeUtc,
    pub created_at: DateTimeUtc,
}

/// A client-safe candidate view.
///
/// It intentionally has no submitted/canonical/effective URL and no free-form
/// error message. The error code is a bounded machine-readable value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverCandidateSnapshot {
    pub candidate_id: Uuid,
    pub release_id: Uuid,
    pub disc_number: u16,
    pub source_kind: CoverSourceKind,
    pub source_release_revision_db_id: Option<i32>,
    pub state: CoverCandidateState,
    pub asset: Option<CoverAssetSnapshot>,
    pub has_remote_url: bool,
    pub attempt_count: u32,
    pub next_attempt_at: Option<DateTimeUtc>,
    pub last_http_status: Option<u16>,
    pub last_error_code: Option<String>,
    pub fetched_at: Option<DateTimeUtc>,
    pub row_version: CoverRowVersion,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverSelectionSnapshot {
    pub selection_id: Uuid,
    pub release_id: Uuid,
    pub disc_number: u16,
    pub candidate_id: Uuid,
    /// The immutable bytes selected at that moment, not a later candidate
    /// lookup. This is the durable selection freeze.
    pub asset: CoverAssetSnapshot,
    pub row_version: CoverRowVersion,
    pub selected_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Clone, PartialEq, Eq)]
pub struct NewCoverCandidate {
    pub candidate_id: Option<Uuid>,
    pub release_id: Uuid,
    /// Zero is release-level artwork; positive values are one-based discs.
    pub disc_number: u16,
    pub source_kind: CoverSourceKind,
    pub source_release_revision_db_id: Option<i32>,
    pub submitted_url: String,
}

impl std::fmt::Debug for NewCoverCandidate {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NewCoverCandidate")
            .field("candidate_id", &self.candidate_id)
            .field("release_id", &self.release_id)
            .field("disc_number", &self.disc_number)
            .field("source_kind", &self.source_kind)
            .field(
                "source_release_revision_db_id",
                &self.source_release_revision_db_id,
            )
            .field("submitted_url", &"[REDACTED]")
            .finish()
    }
}

/// The only public value that contains a fetch URL. It is intended for the
/// background worker and guarded by a short-lived opaque lease token.
#[derive(Clone, PartialEq, Eq)]
pub struct CoverFetchLease {
    pub candidate_id: Uuid,
    pub release_id: Uuid,
    pub disc_number: u16,
    pub source_kind: CoverSourceKind,
    pub request_url: String,
    pub lease_token: Uuid,
    pub lease_expires_at: DateTimeUtc,
    pub attempt_count: u32,
    pub row_version: CoverRowVersion,
}

impl std::fmt::Debug for CoverFetchLease {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CoverFetchLease")
            .field("candidate_id", &self.candidate_id)
            .field("release_id", &self.release_id)
            .field("disc_number", &self.disc_number)
            .field("source_kind", &self.source_kind)
            .field("request_url", &"[REDACTED]")
            .field("lease_token", &"[REDACTED]")
            .field("lease_expires_at", &self.lease_expires_at)
            .field("attempt_count", &self.attempt_count)
            .field("row_version", &self.row_version)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct VerifiedCoverAsset {
    pub content_sha256: Digest,
    pub media_type: CoverMediaType,
    pub width: NonZeroU32,
    pub height: NonZeroU32,
    pub byte_length: u64,
    /// The final URL after redirects. It remains private like the submitted
    /// and canonical forms.
    pub effective_url: Option<String>,
}

impl std::fmt::Debug for VerifiedCoverAsset {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("VerifiedCoverAsset")
            .field("content_sha256", &self.content_sha256)
            .field("media_type", &self.media_type)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("byte_length", &self.byte_length)
            .field(
                "effective_url",
                &self.effective_url.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct CoverFetchFailure {
    pub code: String,
    pub message: Option<String>,
    pub http_status: Option<u16>,
}

impl std::fmt::Debug for CoverFetchFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CoverFetchFailure")
            .field("code", &self.code)
            .field("message", &self.message.as_ref().map(|_| "[REDACTED]"))
            .field("http_status", &self.http_status)
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectCover {
    pub release_id: Uuid,
    pub disc_number: u16,
    pub candidate_id: Uuid,
    /// `None` creates the selection and asserts that no selection exists.
    /// `Some` replaces an existing selection with optimistic locking.
    pub expected_row_version: Option<CoverRowVersion>,
}

#[derive(Debug, Error)]
pub enum CoverError {
    #[error("catalog release {release_id} does not exist")]
    ReleaseNotFound { release_id: Uuid },
    #[error("cover candidate {candidate_id} already exists")]
    CandidateAlreadyExists { candidate_id: Uuid },
    #[error("cover candidate {candidate_id} does not exist")]
    CandidateNotFound { candidate_id: Uuid },
    #[error(
        "cover candidate {candidate_id} changed concurrently: expected {expected}, actual {actual}"
    )]
    CandidateConflict {
        candidate_id: Uuid,
        expected: CoverRowVersion,
        actual: CoverRowVersion,
    },
    #[error("cover candidate {candidate_id} cannot transition from {from} to {to}")]
    InvalidCandidateTransition {
        candidate_id: Uuid,
        from: CoverCandidateState,
        to: CoverCandidateState,
    },
    #[error("cover candidate {candidate_id} is not owned by this worker lease")]
    LeaseMismatch { candidate_id: Uuid },
    #[error("cover candidate {candidate_id} is not verified")]
    CandidateNotVerified { candidate_id: Uuid },
    #[error(
        "cover candidate {candidate_id} belongs to release {actual_release_id} disc {actual_disc_number}, not release {expected_release_id} disc {expected_disc_number}"
    )]
    CandidateScopeMismatch {
        candidate_id: Uuid,
        expected_release_id: Uuid,
        expected_disc_number: u16,
        actual_release_id: Uuid,
        actual_disc_number: u16,
    },
    #[error("cover candidate {candidate_id} does not reference a verified asset")]
    CandidateAssetMissing { candidate_id: Uuid },
    #[error("cover selection for release {release_id} disc {disc_number} already exists")]
    SelectionAlreadyExists { release_id: Uuid, disc_number: u16 },
    #[error("cover selection for release {release_id} disc {disc_number} does not exist")]
    SelectionNotFound { release_id: Uuid, disc_number: u16 },
    #[error(
        "cover selection for release {release_id} disc {disc_number} changed concurrently: expected {expected}, actual {actual}"
    )]
    SelectionConflict {
        release_id: Uuid,
        disc_number: u16,
        expected: CoverRowVersion,
        actual: CoverRowVersion,
    },
    #[error("invalid cover URL: {0}")]
    InvalidUrl(#[source] CoverUrlError),
    #[error("invalid cover {field}: {message}")]
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
    #[error("{entity} {id} has an out-of-range {field}")]
    NumericOutOfRange {
        entity: &'static str,
        id: Uuid,
        field: &'static str,
    },
    #[error("cover asset digest {digest} conflicts with previously verified metadata")]
    AssetDigestConflict { digest: Digest },
    #[error(transparent)]
    Database(#[from] DbErr),
}

#[derive(Clone)]
pub struct CoverRepository {
    database: DatabaseConnection,
}

impl CoverRepository {
    pub fn new(database: DatabaseConnection) -> Self {
        Self { database }
    }

    pub async fn create_candidate(
        &self,
        input: NewCoverCandidate,
    ) -> Result<CoverCandidateSnapshot, CoverError> {
        let release = get_release_model(&self.database, input.release_id)
            .await?
            .ok_or(CoverError::ReleaseNotFound {
                release_id: input.release_id,
            })?;
        let disc_number = store_disc_number(input.disc_number, input.release_id)?;
        if input.submitted_url.is_empty() {
            return Err(CoverError::InvalidInput {
                field: "submitted_url",
                message: "value must not be empty",
            });
        }
        let canonical_url = preferred_url(input.source_kind, &input.submitted_url)?;
        let candidate_id = input.candidate_id.unwrap_or_else(Uuid::new_v4);
        let result = cover_candidate::Entity::insert(cover_candidate::ActiveModel {
            id: NotSet,
            candidate_id: Set(candidate_id),
            release_db_id: Set(release.id),
            disc_number: Set(disc_number),
            source_kind: Set(input.source_kind.as_str().to_owned()),
            source_release_revision_db_id: Set(input.source_release_revision_db_id),
            submitted_url: Set(Some(input.submitted_url)),
            canonical_url: Set(Some(canonical_url)),
            effective_url: Set(None),
            state: Set(CoverCandidateState::Discovered.as_str().to_owned()),
            asset_db_id: Set(None),
            attempt_count: Set(0),
            lease_token: Set(None),
            lease_expires_at: Set(None),
            next_attempt_at: Set(None),
            last_http_status: Set(None),
            last_error_code: Set(None),
            last_error_message: Set(None),
            fetched_at: Set(None),
            row_version: Set(1),
            created_at: NotSet,
            updated_at: NotSet,
        })
        .on_conflict(
            OnConflict::column(cover_candidate::Column::CandidateId)
                .do_nothing()
                .to_owned(),
        )
        .do_nothing()
        .exec_without_returning(&self.database)
        .await?;
        match result {
            TryInsertResult::Inserted(1) => self
                .get_candidate(candidate_id)
                .await?
                .ok_or(CoverError::CandidateNotFound { candidate_id }),
            TryInsertResult::Inserted(0) | TryInsertResult::Conflicted => {
                Err(CoverError::CandidateAlreadyExists { candidate_id })
            }
            TryInsertResult::Inserted(_) | TryInsertResult::Empty => Err(CoverError::Database(
                DbErr::Custom("cover candidate insert affected an unexpected row count".into()),
            )),
        }
    }

    pub async fn get_candidate(
        &self,
        candidate_id: Uuid,
    ) -> Result<Option<CoverCandidateSnapshot>, CoverError> {
        let Some(candidate) = get_candidate_model(&self.database, candidate_id).await? else {
            return Ok(None);
        };
        candidate_to_snapshot(&self.database, candidate)
            .await
            .map(Some)
    }

    pub async fn list_candidates(
        &self,
        release_id: Uuid,
        disc_number: Option<u16>,
    ) -> Result<Vec<CoverCandidateSnapshot>, CoverError> {
        let release = get_release_model(&self.database, release_id)
            .await?
            .ok_or(CoverError::ReleaseNotFound { release_id })?;
        let mut query = cover_candidate::Entity::find()
            .filter(cover_candidate::Column::ReleaseDbId.eq(release.id));
        if let Some(disc_number) = disc_number {
            query = query.filter(
                cover_candidate::Column::DiscNumber.eq(store_disc_number(disc_number, release_id)?),
            );
        }
        let models = query
            .order_by_asc(cover_candidate::Column::DiscNumber)
            .order_by_asc(cover_candidate::Column::Id)
            .all(&self.database)
            .await?;
        let mut snapshots = Vec::with_capacity(models.len());
        for model in models {
            snapshots.push(candidate_to_snapshot(&self.database, model).await?);
        }
        Ok(snapshots)
    }

    pub async fn queue_candidate(
        &self,
        candidate_id: Uuid,
        expected: CoverRowVersion,
        not_before: Option<DateTimeUtc>,
    ) -> Result<CoverCandidateSnapshot, CoverError> {
        let current = get_candidate_model(&self.database, candidate_id)
            .await?
            .ok_or(CoverError::CandidateNotFound { candidate_id })?;
        ensure_candidate_version(&current, expected)?;
        let state = parse_candidate_state(candidate_id, &current.state)?;
        if !state.can_transition_to(CoverCandidateState::Queued) {
            return Err(CoverError::InvalidCandidateTransition {
                candidate_id,
                from: state,
                to: CoverCandidateState::Queued,
            });
        }
        let next = expected.next("cover candidate", candidate_id)?;
        let result = cover_candidate::Entity::update_many()
            .col_expr(
                cover_candidate::Column::State,
                Expr::value(CoverCandidateState::Queued.as_str()),
            )
            .col_expr(
                cover_candidate::Column::LeaseToken,
                Expr::value(None::<Uuid>),
            )
            .col_expr(
                cover_candidate::Column::LeaseExpiresAt,
                Expr::value(None::<StoredTimestamp>),
            )
            .col_expr(
                cover_candidate::Column::NextAttemptAt,
                Expr::value(not_before.map(store_timestamp)),
            )
            .col_expr(
                cover_candidate::Column::RowVersion,
                Expr::value(next.as_i64("cover candidate", candidate_id)?),
            )
            .col_expr(
                cover_candidate::Column::UpdatedAt,
                Expr::current_timestamp().into(),
            )
            .filter(cover_candidate::Column::CandidateId.eq(candidate_id))
            .filter(
                cover_candidate::Column::RowVersion
                    .eq(expected.as_i64("cover candidate", candidate_id)?),
            )
            .exec(&self.database)
            .await?;
        if result.rows_affected != 1 {
            return self
                .candidate_conflict_or_not_found(candidate_id, expected)
                .await;
        }
        self.get_candidate(candidate_id)
            .await?
            .ok_or(CoverError::CandidateNotFound { candidate_id })
    }

    /// Claim one due candidate using a compare-and-swap update. An expired
    /// fetching lease is reclaimable, which prevents a dead worker from
    /// permanently wedging the queue.
    pub async fn claim_next(
        &self,
        lease_for: Duration,
    ) -> Result<Option<CoverFetchLease>, CoverError> {
        self.claim_next_at(chrono::Utc::now(), lease_for).await
    }

    pub async fn claim_next_at(
        &self,
        claimed_at: DateTimeUtc,
        lease_for: Duration,
    ) -> Result<Option<CoverFetchLease>, CoverError> {
        if lease_for.is_zero() {
            return Err(CoverError::InvalidInput {
                field: "lease_for",
                message: "duration must be positive",
            });
        }
        let lease_delta =
            chrono::Duration::from_std(lease_for).map_err(|_| CoverError::InvalidInput {
                field: "lease_for",
                message: "duration is too large",
            })?;
        let lease_expires_at =
            claimed_at
                .checked_add_signed(lease_delta)
                .ok_or(CoverError::InvalidInput {
                    field: "lease_for",
                    message: "duration overflows timestamp",
                })?;
        let stored_now = store_timestamp(claimed_at);

        for _ in 0..MAX_CLAIM_CAS_ATTEMPTS {
            let queued_due = Condition::all()
                .add(cover_candidate::Column::State.eq(CoverCandidateState::Queued.as_str()))
                .add(
                    Condition::any()
                        .add(cover_candidate::Column::NextAttemptAt.is_null())
                        .add(cover_candidate::Column::NextAttemptAt.lte(stored_now)),
                );
            let expired_fetch = Condition::all()
                .add(cover_candidate::Column::State.eq(CoverCandidateState::Fetching.as_str()))
                .add(cover_candidate::Column::LeaseExpiresAt.lte(stored_now));
            let Some(candidate) = cover_candidate::Entity::find()
                .filter(Condition::any().add(queued_due).add(expired_fetch))
                .order_by_asc(cover_candidate::Column::NextAttemptAt)
                .order_by_asc(cover_candidate::Column::Id)
                .one(&self.database)
                .await?
            else {
                return Ok(None);
            };
            let candidate_id = candidate.candidate_id;
            // Validate persisted values before changing queue state. A corrupt
            // row should stay inspectable instead of being leased and then
            // abandoned until its timeout.
            let release = catalog_release::Entity::find_by_id(candidate.release_db_id)
                .one(&self.database)
                .await?
                .ok_or_else(|| missing_reference("cover candidate", candidate_id, "release"))?;
            let request_url = candidate
                .effective_url
                .clone()
                .or_else(|| candidate.canonical_url.clone())
                .or_else(|| candidate.submitted_url.clone())
                .ok_or(CoverError::InvalidPersistedValue {
                    entity: "cover candidate",
                    id: candidate_id,
                    field: "request_url",
                    value: "missing".to_owned(),
                })?;
            let disc_number = load_disc_number(candidate_id, candidate.disc_number)?;
            let source_kind = parse_source_kind(candidate_id, &candidate.source_kind)?;
            let current_version =
                parse_row_version("cover candidate", candidate_id, candidate.row_version)?;
            let next_version = current_version.next("cover candidate", candidate_id)?;
            let attempt_count =
                candidate
                    .attempt_count
                    .checked_add(1)
                    .ok_or(CoverError::NumericOutOfRange {
                        entity: "cover candidate",
                        id: candidate_id,
                        field: "attempt_count",
                    })?;
            let lease_token = Uuid::new_v4();
            let result = cover_candidate::Entity::update_many()
                .col_expr(
                    cover_candidate::Column::State,
                    Expr::value(CoverCandidateState::Fetching.as_str()),
                )
                .col_expr(
                    cover_candidate::Column::AttemptCount,
                    Expr::value(attempt_count),
                )
                .col_expr(
                    cover_candidate::Column::LeaseToken,
                    Expr::value(Some(lease_token)),
                )
                .col_expr(
                    cover_candidate::Column::LeaseExpiresAt,
                    Expr::value(Some(store_timestamp(lease_expires_at))),
                )
                .col_expr(
                    cover_candidate::Column::NextAttemptAt,
                    Expr::value(None::<StoredTimestamp>),
                )
                .col_expr(
                    cover_candidate::Column::RowVersion,
                    Expr::value(next_version.as_i64("cover candidate", candidate_id)?),
                )
                .col_expr(
                    cover_candidate::Column::UpdatedAt,
                    Expr::current_timestamp().into(),
                )
                .filter(cover_candidate::Column::Id.eq(candidate.id))
                .filter(cover_candidate::Column::RowVersion.eq(candidate.row_version))
                .exec(&self.database)
                .await?;
            if result.rows_affected != 1 {
                continue;
            }

            return Ok(Some(CoverFetchLease {
                candidate_id,
                release_id: release.release_id,
                disc_number,
                source_kind,
                request_url,
                lease_token,
                lease_expires_at,
                attempt_count: u32::try_from(attempt_count).map_err(|_| {
                    CoverError::NumericOutOfRange {
                        entity: "cover candidate",
                        id: candidate_id,
                        field: "attempt_count",
                    }
                })?,
                row_version: next_version,
            }));
        }
        // Repeated CAS misses indicate contention, not absence. Returning no
        // lease lets the worker poll again without turning contention into an
        // operational error.
        Ok(None)
    }

    pub async fn complete_fetch(
        &self,
        candidate_id: Uuid,
        expected: CoverRowVersion,
        lease_token: Uuid,
        verified: VerifiedCoverAsset,
    ) -> Result<CoverCandidateSnapshot, CoverError> {
        if verified.byte_length == 0 {
            return Err(CoverError::InvalidInput {
                field: "byte_length",
                message: "value must be positive",
            });
        }
        if let Some(effective_url) = &verified.effective_url {
            // Validation is intentional; the exact effective value is stored
            // below, not the potentially rewritten validation result.
            canonicalize_cover_url(effective_url).map_err(CoverError::InvalidUrl)?;
        }

        let transaction = self.database.begin().await?;
        let candidate = get_candidate_model(&transaction, candidate_id)
            .await?
            .ok_or(CoverError::CandidateNotFound { candidate_id })?;
        ensure_worker_lease(
            &candidate,
            expected,
            lease_token,
            CoverCandidateState::Verified,
        )?;
        let asset = get_or_insert_asset(&transaction, &verified).await?;
        let next = expected.next("cover candidate", candidate_id)?;
        let effective_url = verified.effective_url.or(candidate.effective_url.clone());
        let result = cover_candidate::Entity::update_many()
            .col_expr(
                cover_candidate::Column::State,
                Expr::value(CoverCandidateState::Verified.as_str()),
            )
            .col_expr(
                cover_candidate::Column::AssetDbId,
                Expr::value(Some(asset.id)),
            )
            .col_expr(
                cover_candidate::Column::EffectiveUrl,
                Expr::value(effective_url),
            )
            .col_expr(
                cover_candidate::Column::LeaseToken,
                Expr::value(None::<Uuid>),
            )
            .col_expr(
                cover_candidate::Column::LeaseExpiresAt,
                Expr::value(None::<StoredTimestamp>),
            )
            .col_expr(
                cover_candidate::Column::NextAttemptAt,
                Expr::value(None::<StoredTimestamp>),
            )
            .col_expr(
                cover_candidate::Column::LastHttpStatus,
                Expr::value(None::<i32>),
            )
            .col_expr(
                cover_candidate::Column::LastErrorCode,
                Expr::value(None::<String>),
            )
            .col_expr(
                cover_candidate::Column::LastErrorMessage,
                Expr::value(None::<String>),
            )
            .col_expr(cover_candidate::Column::FetchedAt, Expr::value(Some(now())))
            .col_expr(
                cover_candidate::Column::RowVersion,
                Expr::value(next.as_i64("cover candidate", candidate_id)?),
            )
            .col_expr(
                cover_candidate::Column::UpdatedAt,
                Expr::current_timestamp().into(),
            )
            .filter(cover_candidate::Column::Id.eq(candidate.id))
            .filter(cover_candidate::Column::RowVersion.eq(candidate.row_version))
            .filter(cover_candidate::Column::LeaseToken.eq(Some(lease_token)))
            .exec(&transaction)
            .await?;
        if result.rows_affected != 1 {
            return Err(CoverError::LeaseMismatch { candidate_id });
        }
        transaction.commit().await?;
        self.get_candidate(candidate_id)
            .await?
            .ok_or(CoverError::CandidateNotFound { candidate_id })
    }

    pub async fn retry_fetch(
        &self,
        candidate_id: Uuid,
        expected: CoverRowVersion,
        lease_token: Uuid,
        failure: CoverFetchFailure,
        not_before: DateTimeUtc,
    ) -> Result<CoverCandidateSnapshot, CoverError> {
        self.finish_with_failure(
            candidate_id,
            expected,
            lease_token,
            failure,
            CoverCandidateState::Queued,
            Some(not_before),
        )
        .await
    }

    pub async fn reject_fetch(
        &self,
        candidate_id: Uuid,
        expected: CoverRowVersion,
        lease_token: Uuid,
        failure: CoverFetchFailure,
    ) -> Result<CoverCandidateSnapshot, CoverError> {
        self.finish_with_failure(
            candidate_id,
            expected,
            lease_token,
            failure,
            CoverCandidateState::Rejected,
            None,
        )
        .await
    }

    async fn finish_with_failure(
        &self,
        candidate_id: Uuid,
        expected: CoverRowVersion,
        lease_token: Uuid,
        failure: CoverFetchFailure,
        target: CoverCandidateState,
        not_before: Option<DateTimeUtc>,
    ) -> Result<CoverCandidateSnapshot, CoverError> {
        validate_failure(&failure)?;
        let candidate = get_candidate_model(&self.database, candidate_id)
            .await?
            .ok_or(CoverError::CandidateNotFound { candidate_id })?;
        ensure_worker_lease(&candidate, expected, lease_token, target)?;
        let next = expected.next("cover candidate", candidate_id)?;
        let status = failure.http_status.map(i32::from);
        let safe_message = failure.message.as_deref().and_then(sanitize_message);
        let result = cover_candidate::Entity::update_many()
            .col_expr(cover_candidate::Column::State, Expr::value(target.as_str()))
            .col_expr(
                cover_candidate::Column::LeaseToken,
                Expr::value(None::<Uuid>),
            )
            .col_expr(
                cover_candidate::Column::LeaseExpiresAt,
                Expr::value(None::<StoredTimestamp>),
            )
            .col_expr(
                cover_candidate::Column::NextAttemptAt,
                Expr::value(not_before.map(store_timestamp)),
            )
            .col_expr(cover_candidate::Column::LastHttpStatus, Expr::value(status))
            .col_expr(
                cover_candidate::Column::LastErrorCode,
                Expr::value(Some(failure.code)),
            )
            .col_expr(
                cover_candidate::Column::LastErrorMessage,
                Expr::value(safe_message),
            )
            .col_expr(
                cover_candidate::Column::RowVersion,
                Expr::value(next.as_i64("cover candidate", candidate_id)?),
            )
            .col_expr(
                cover_candidate::Column::UpdatedAt,
                Expr::current_timestamp().into(),
            )
            .filter(cover_candidate::Column::Id.eq(candidate.id))
            .filter(cover_candidate::Column::RowVersion.eq(candidate.row_version))
            .filter(cover_candidate::Column::LeaseToken.eq(Some(lease_token)))
            .exec(&self.database)
            .await?;
        if result.rows_affected != 1 {
            return Err(CoverError::LeaseMismatch { candidate_id });
        }
        self.get_candidate(candidate_id)
            .await?
            .ok_or(CoverError::CandidateNotFound { candidate_id })
    }

    pub async fn reject_candidate(
        &self,
        candidate_id: Uuid,
        expected: CoverRowVersion,
        reason_code: String,
    ) -> Result<CoverCandidateSnapshot, CoverError> {
        validate_failure_code(&reason_code)?;
        let candidate = get_candidate_model(&self.database, candidate_id)
            .await?
            .ok_or(CoverError::CandidateNotFound { candidate_id })?;
        ensure_candidate_version(&candidate, expected)?;
        let state = parse_candidate_state(candidate_id, &candidate.state)?;
        // A fetching candidate can only be rejected by the lease owner.
        if state == CoverCandidateState::Fetching
            || !state.can_transition_to(CoverCandidateState::Rejected)
        {
            return Err(CoverError::InvalidCandidateTransition {
                candidate_id,
                from: state,
                to: CoverCandidateState::Rejected,
            });
        }
        let next = expected.next("cover candidate", candidate_id)?;
        let result = cover_candidate::Entity::update_many()
            .col_expr(
                cover_candidate::Column::State,
                Expr::value(CoverCandidateState::Rejected.as_str()),
            )
            .col_expr(
                cover_candidate::Column::LastErrorCode,
                Expr::value(Some(reason_code)),
            )
            .col_expr(
                cover_candidate::Column::LastErrorMessage,
                Expr::value(None::<String>),
            )
            .col_expr(
                cover_candidate::Column::NextAttemptAt,
                Expr::value(None::<StoredTimestamp>),
            )
            .col_expr(
                cover_candidate::Column::RowVersion,
                Expr::value(next.as_i64("cover candidate", candidate_id)?),
            )
            .col_expr(
                cover_candidate::Column::UpdatedAt,
                Expr::current_timestamp().into(),
            )
            .filter(cover_candidate::Column::Id.eq(candidate.id))
            .filter(cover_candidate::Column::RowVersion.eq(candidate.row_version))
            .exec(&self.database)
            .await?;
        if result.rows_affected != 1 {
            return self
                .candidate_conflict_or_not_found(candidate_id, expected)
                .await;
        }
        self.get_candidate(candidate_id)
            .await?
            .ok_or(CoverError::CandidateNotFound { candidate_id })
    }

    pub async fn select_cover(
        &self,
        input: SelectCover,
    ) -> Result<CoverSelectionSnapshot, CoverError> {
        let transaction = self.database.begin().await?;
        let release = get_release_model(&transaction, input.release_id)
            .await?
            .ok_or(CoverError::ReleaseNotFound {
                release_id: input.release_id,
            })?;
        let disc_number = store_disc_number(input.disc_number, input.release_id)?;
        let candidate = get_candidate_model(&transaction, input.candidate_id)
            .await?
            .ok_or(CoverError::CandidateNotFound {
                candidate_id: input.candidate_id,
            })?;
        let actual_release = catalog_release::Entity::find_by_id(candidate.release_db_id)
            .one(&transaction)
            .await?
            .ok_or_else(|| missing_reference("cover candidate", input.candidate_id, "release"))?;
        let actual_disc = load_disc_number(input.candidate_id, candidate.disc_number)?;
        if candidate.release_db_id != release.id || candidate.disc_number != disc_number {
            return Err(CoverError::CandidateScopeMismatch {
                candidate_id: input.candidate_id,
                expected_release_id: input.release_id,
                expected_disc_number: input.disc_number,
                actual_release_id: actual_release.release_id,
                actual_disc_number: actual_disc,
            });
        }
        if parse_candidate_state(input.candidate_id, &candidate.state)?
            != CoverCandidateState::Verified
        {
            return Err(CoverError::CandidateNotVerified {
                candidate_id: input.candidate_id,
            });
        }
        let asset_db_id = candidate
            .asset_db_id
            .ok_or(CoverError::CandidateAssetMissing {
                candidate_id: input.candidate_id,
            })?;
        cover_asset::Entity::find_by_id(asset_db_id)
            .one(&transaction)
            .await?
            .ok_or_else(|| missing_reference("cover candidate", input.candidate_id, "asset"))?;

        let existing = cover_selection::Entity::find()
            .filter(cover_selection::Column::ReleaseDbId.eq(release.id))
            .filter(cover_selection::Column::DiscNumber.eq(disc_number))
            .one(&transaction)
            .await?;
        match (existing, input.expected_row_version) {
            (None, Some(_)) => {
                return Err(CoverError::SelectionNotFound {
                    release_id: input.release_id,
                    disc_number: input.disc_number,
                });
            }
            (None, None) => {
                let selection_id = Uuid::new_v4();
                let inserted = cover_selection::Entity::insert(cover_selection::ActiveModel {
                    id: NotSet,
                    selection_id: Set(selection_id),
                    release_db_id: Set(release.id),
                    disc_number: Set(disc_number),
                    candidate_db_id: Set(candidate.id),
                    asset_db_id: Set(asset_db_id),
                    row_version: Set(1),
                    selected_at: Set(now()),
                    updated_at: NotSet,
                })
                .on_conflict(
                    OnConflict::columns([
                        cover_selection::Column::ReleaseDbId,
                        cover_selection::Column::DiscNumber,
                    ])
                    .do_nothing()
                    .to_owned(),
                )
                .do_nothing()
                .exec_without_returning(&transaction)
                .await?;
                if !matches!(inserted, TryInsertResult::Inserted(1)) {
                    return Err(CoverError::SelectionAlreadyExists {
                        release_id: input.release_id,
                        disc_number: input.disc_number,
                    });
                }
            }
            (Some(_), None) => {
                return Err(CoverError::SelectionAlreadyExists {
                    release_id: input.release_id,
                    disc_number: input.disc_number,
                });
            }
            (Some(selection), Some(expected)) => {
                let actual = parse_row_version(
                    "cover selection",
                    selection.selection_id,
                    selection.row_version,
                )?;
                if actual != expected {
                    return Err(CoverError::SelectionConflict {
                        release_id: input.release_id,
                        disc_number: input.disc_number,
                        expected,
                        actual,
                    });
                }
                let next = expected.next("cover selection", selection.selection_id)?;
                let result = cover_selection::Entity::update_many()
                    .col_expr(
                        cover_selection::Column::CandidateDbId,
                        Expr::value(candidate.id),
                    )
                    .col_expr(cover_selection::Column::AssetDbId, Expr::value(asset_db_id))
                    .col_expr(
                        cover_selection::Column::RowVersion,
                        Expr::value(next.as_i64("cover selection", selection.selection_id)?),
                    )
                    .col_expr(
                        cover_selection::Column::SelectedAt,
                        Expr::current_timestamp().into(),
                    )
                    .col_expr(
                        cover_selection::Column::UpdatedAt,
                        Expr::current_timestamp().into(),
                    )
                    .filter(cover_selection::Column::Id.eq(selection.id))
                    .filter(cover_selection::Column::RowVersion.eq(selection.row_version))
                    .exec(&transaction)
                    .await?;
                if result.rows_affected != 1 {
                    let current = cover_selection::Entity::find_by_id(selection.id)
                        .one(&transaction)
                        .await?;
                    return match current {
                        Some(current) => Err(CoverError::SelectionConflict {
                            release_id: input.release_id,
                            disc_number: input.disc_number,
                            expected,
                            actual: parse_row_version(
                                "cover selection",
                                current.selection_id,
                                current.row_version,
                            )?,
                        }),
                        None => Err(CoverError::SelectionNotFound {
                            release_id: input.release_id,
                            disc_number: input.disc_number,
                        }),
                    };
                }
            }
        }
        transaction.commit().await?;
        self.get_selection(input.release_id, input.disc_number)
            .await?
            .ok_or(CoverError::SelectionNotFound {
                release_id: input.release_id,
                disc_number: input.disc_number,
            })
    }

    pub async fn get_selection(
        &self,
        release_id: Uuid,
        disc_number: u16,
    ) -> Result<Option<CoverSelectionSnapshot>, CoverError> {
        let Some(release) = get_release_model(&self.database, release_id).await? else {
            return Ok(None);
        };
        let stored_disc = store_disc_number(disc_number, release_id)?;
        let Some(selection) = cover_selection::Entity::find()
            .filter(cover_selection::Column::ReleaseDbId.eq(release.id))
            .filter(cover_selection::Column::DiscNumber.eq(stored_disc))
            .one(&self.database)
            .await?
        else {
            return Ok(None);
        };
        selection_to_snapshot(&self.database, release_id, selection)
            .await
            .map(Some)
    }

    async fn candidate_conflict_or_not_found(
        &self,
        candidate_id: Uuid,
        expected: CoverRowVersion,
    ) -> Result<CoverCandidateSnapshot, CoverError> {
        match self.get_candidate(candidate_id).await? {
            Some(actual) => Err(CoverError::CandidateConflict {
                candidate_id,
                expected,
                actual: actual.row_version,
            }),
            None => Err(CoverError::CandidateNotFound { candidate_id }),
        }
    }
}

/// Application-facing facade. Keeping workers and web resolvers on this one
/// facade prevents either surface from bypassing repository invariants.
#[derive(Clone)]
pub struct CoverService {
    repository: CoverRepository,
}

impl CoverService {
    pub fn new(repository: CoverRepository) -> Self {
        Self { repository }
    }

    pub const fn repository(&self) -> &CoverRepository {
        &self.repository
    }

    pub async fn create_candidate(
        &self,
        input: NewCoverCandidate,
    ) -> Result<CoverCandidateSnapshot, CoverError> {
        self.repository.create_candidate(input).await
    }

    pub async fn get_candidate(
        &self,
        candidate_id: Uuid,
    ) -> Result<Option<CoverCandidateSnapshot>, CoverError> {
        self.repository.get_candidate(candidate_id).await
    }

    pub async fn list_candidates(
        &self,
        release_id: Uuid,
        disc_number: Option<u16>,
    ) -> Result<Vec<CoverCandidateSnapshot>, CoverError> {
        self.repository
            .list_candidates(release_id, disc_number)
            .await
    }

    pub async fn queue_candidate(
        &self,
        candidate_id: Uuid,
        expected: CoverRowVersion,
        not_before: Option<DateTimeUtc>,
    ) -> Result<CoverCandidateSnapshot, CoverError> {
        self.repository
            .queue_candidate(candidate_id, expected, not_before)
            .await
    }

    pub async fn claim_next(
        &self,
        lease_for: Duration,
    ) -> Result<Option<CoverFetchLease>, CoverError> {
        self.repository.claim_next(lease_for).await
    }

    pub async fn complete_fetch(
        &self,
        candidate_id: Uuid,
        expected: CoverRowVersion,
        lease_token: Uuid,
        verified: VerifiedCoverAsset,
    ) -> Result<CoverCandidateSnapshot, CoverError> {
        self.repository
            .complete_fetch(candidate_id, expected, lease_token, verified)
            .await
    }

    pub async fn retry_fetch(
        &self,
        candidate_id: Uuid,
        expected: CoverRowVersion,
        lease_token: Uuid,
        failure: CoverFetchFailure,
        not_before: DateTimeUtc,
    ) -> Result<CoverCandidateSnapshot, CoverError> {
        self.repository
            .retry_fetch(candidate_id, expected, lease_token, failure, not_before)
            .await
    }

    pub async fn reject_fetch(
        &self,
        candidate_id: Uuid,
        expected: CoverRowVersion,
        lease_token: Uuid,
        failure: CoverFetchFailure,
    ) -> Result<CoverCandidateSnapshot, CoverError> {
        self.repository
            .reject_fetch(candidate_id, expected, lease_token, failure)
            .await
    }

    pub async fn reject_candidate(
        &self,
        candidate_id: Uuid,
        expected: CoverRowVersion,
        reason_code: String,
    ) -> Result<CoverCandidateSnapshot, CoverError> {
        self.repository
            .reject_candidate(candidate_id, expected, reason_code)
            .await
    }

    pub async fn select_cover(
        &self,
        input: SelectCover,
    ) -> Result<CoverSelectionSnapshot, CoverError> {
        self.repository.select_cover(input).await
    }

    pub async fn get_selection(
        &self,
        release_id: Uuid,
        disc_number: u16,
    ) -> Result<Option<CoverSelectionSnapshot>, CoverError> {
        self.repository.get_selection(release_id, disc_number).await
    }
}

async fn get_release_model<C: ConnectionTrait>(
    connection: &C,
    release_id: Uuid,
) -> Result<Option<catalog_release::Model>, CoverError> {
    Ok(catalog_release::Entity::find()
        .filter(catalog_release::Column::ReleaseId.eq(release_id))
        .one(connection)
        .await?)
}

async fn get_candidate_model<C: ConnectionTrait>(
    connection: &C,
    candidate_id: Uuid,
) -> Result<Option<cover_candidate::Model>, CoverError> {
    Ok(cover_candidate::Entity::find()
        .filter(cover_candidate::Column::CandidateId.eq(candidate_id))
        .one(connection)
        .await?)
}

async fn candidate_to_snapshot<C: ConnectionTrait>(
    connection: &C,
    model: cover_candidate::Model,
) -> Result<CoverCandidateSnapshot, CoverError> {
    let candidate_id = model.candidate_id;
    let release = catalog_release::Entity::find_by_id(model.release_db_id)
        .one(connection)
        .await?
        .ok_or_else(|| missing_reference("cover candidate", candidate_id, "release"))?;
    let asset = match model.asset_db_id {
        Some(asset_id) => Some(
            cover_asset::Entity::find_by_id(asset_id)
                .one(connection)
                .await?
                .ok_or_else(|| missing_reference("cover candidate", candidate_id, "asset"))?
                .try_into()?,
        ),
        None => None,
    };
    Ok(CoverCandidateSnapshot {
        candidate_id,
        release_id: release.release_id,
        disc_number: load_disc_number(candidate_id, model.disc_number)?,
        source_kind: parse_source_kind(candidate_id, &model.source_kind)?,
        source_release_revision_db_id: model.source_release_revision_db_id,
        state: parse_candidate_state(candidate_id, &model.state)?,
        asset,
        has_remote_url: model.effective_url.is_some()
            || model.canonical_url.is_some()
            || model.submitted_url.is_some(),
        attempt_count: u32::try_from(model.attempt_count).map_err(|_| {
            CoverError::InvalidPersistedValue {
                entity: "cover candidate",
                id: candidate_id,
                field: "attempt_count",
                value: model.attempt_count.to_string(),
            }
        })?,
        next_attempt_at: model.next_attempt_at.map(timestamp),
        last_http_status: model
            .last_http_status
            .map(|value| {
                u16::try_from(value).map_err(|_| CoverError::InvalidPersistedValue {
                    entity: "cover candidate",
                    id: candidate_id,
                    field: "last_http_status",
                    value: value.to_string(),
                })
            })
            .transpose()?,
        last_error_code: model.last_error_code,
        fetched_at: model.fetched_at.map(timestamp),
        row_version: parse_row_version("cover candidate", candidate_id, model.row_version)?,
        created_at: timestamp(model.created_at),
        updated_at: timestamp(model.updated_at),
    })
}

async fn selection_to_snapshot<C: ConnectionTrait>(
    connection: &C,
    release_id: Uuid,
    model: cover_selection::Model,
) -> Result<CoverSelectionSnapshot, CoverError> {
    let selection_id = model.selection_id;
    let candidate = cover_candidate::Entity::find_by_id(model.candidate_db_id)
        .one(connection)
        .await?
        .ok_or_else(|| missing_reference("cover selection", selection_id, "candidate"))?;
    let asset: CoverAssetSnapshot = cover_asset::Entity::find_by_id(model.asset_db_id)
        .one(connection)
        .await?
        .ok_or_else(|| missing_reference("cover selection", selection_id, "asset"))?
        .try_into()?;
    Ok(CoverSelectionSnapshot {
        selection_id,
        release_id,
        disc_number: load_disc_number(selection_id, model.disc_number)?,
        candidate_id: candidate.candidate_id,
        asset,
        row_version: parse_row_version("cover selection", selection_id, model.row_version)?,
        selected_at: timestamp(model.selected_at),
        updated_at: timestamp(model.updated_at),
    })
}

impl TryFrom<cover_asset::Model> for CoverAssetSnapshot {
    type Error = CoverError;

    fn try_from(model: cover_asset::Model) -> Result<Self, Self::Error> {
        let asset_id = model.asset_id;
        let digest = digest_from_bytes(asset_id, model.content_sha256)?;
        let media_type = CoverMediaType::from_str(&model.media_type).map_err(|_| {
            CoverError::InvalidPersistedValue {
                entity: "cover asset",
                id: asset_id,
                field: "media_type",
                value: model.media_type.clone(),
            }
        })?;
        let expected_key = cover_asset_storage_key(digest.as_bytes(), media_type);
        if model.storage_key != expected_key {
            return Err(CoverError::InvalidPersistedValue {
                entity: "cover asset",
                id: asset_id,
                field: "storage_key",
                value: "does not match digest and media type".to_owned(),
            });
        }
        Ok(Self {
            asset_id,
            content_sha256: digest,
            storage_key: model.storage_key,
            media_type,
            width: positive_i32(asset_id, "width", model.width)?,
            height: positive_i32(asset_id, "height", model.height)?,
            byte_length: positive_i64(asset_id, "byte_length", model.byte_length)?,
            fetched_at: timestamp(model.fetched_at),
            verified_at: timestamp(model.verified_at),
            created_at: timestamp(model.created_at),
        })
    }
}

async fn get_or_insert_asset<C: ConnectionTrait>(
    connection: &C,
    verified: &VerifiedCoverAsset,
) -> Result<cover_asset::Model, CoverError> {
    let digest = verified.content_sha256;
    let digest_bytes = digest.as_bytes().to_vec();
    let storage_key = cover_asset_storage_key(digest.as_bytes(), verified.media_type);
    let width = i32::try_from(verified.width.get()).map_err(|_| CoverError::NumericOutOfRange {
        entity: "cover asset",
        id: Uuid::nil(),
        field: "width",
    })?;
    let height =
        i32::try_from(verified.height.get()).map_err(|_| CoverError::NumericOutOfRange {
            entity: "cover asset",
            id: Uuid::nil(),
            field: "height",
        })?;
    let byte_length =
        i64::try_from(verified.byte_length).map_err(|_| CoverError::NumericOutOfRange {
            entity: "cover asset",
            id: Uuid::nil(),
            field: "byte_length",
        })?;

    if let Some(existing) = cover_asset::Entity::find()
        .filter(cover_asset::Column::ContentSha256.eq(digest_bytes.clone()))
        .one(connection)
        .await?
    {
        ensure_asset_identity(
            &existing,
            digest,
            &storage_key,
            verified.media_type,
            width,
            height,
            byte_length,
        )?;
        return Ok(existing);
    }

    let asset_id = Uuid::new_v4();
    let inserted = cover_asset::Entity::insert(cover_asset::ActiveModel {
        id: NotSet,
        asset_id: Set(asset_id),
        content_sha256: Set(digest_bytes.clone()),
        storage_key: Set(storage_key.clone()),
        media_type: Set(verified.media_type.as_str().to_owned()),
        width: Set(width),
        height: Set(height),
        byte_length: Set(byte_length),
        fetched_at: Set(now()),
        verified_at: Set(now()),
        created_at: NotSet,
    })
    .on_conflict(
        OnConflict::column(cover_asset::Column::ContentSha256)
            .do_nothing()
            .to_owned(),
    )
    .do_nothing()
    .exec_without_returning(connection)
    .await?;
    match inserted {
        TryInsertResult::Inserted(1)
        | TryInsertResult::Inserted(0)
        | TryInsertResult::Conflicted => {
            let existing = cover_asset::Entity::find()
                .filter(cover_asset::Column::ContentSha256.eq(digest_bytes))
                .one(connection)
                .await?
                .ok_or_else(|| {
                    CoverError::Database(DbErr::Custom(
                        "content-addressed cover insert completed without a readable row".into(),
                    ))
                })?;
            ensure_asset_identity(
                &existing,
                digest,
                &storage_key,
                verified.media_type,
                width,
                height,
                byte_length,
            )?;
            Ok(existing)
        }
        TryInsertResult::Inserted(_) | TryInsertResult::Empty => Err(CoverError::Database(
            DbErr::Custom("cover asset insert affected an unexpected row count".into()),
        )),
    }
}

#[allow(clippy::too_many_arguments)]
fn ensure_asset_identity(
    model: &cover_asset::Model,
    digest: Digest,
    storage_key: &str,
    media_type: CoverMediaType,
    width: i32,
    height: i32,
    byte_length: i64,
) -> Result<(), CoverError> {
    if model.storage_key != storage_key
        || model.media_type != media_type.as_str()
        || model.width != width
        || model.height != height
        || model.byte_length != byte_length
    {
        Err(CoverError::AssetDigestConflict { digest })
    } else {
        Ok(())
    }
}

fn preferred_url(source: CoverSourceKind, submitted: &str) -> Result<String, CoverError> {
    match source {
        CoverSourceKind::Amazon => preferred_amazon_artwork_url(submitted),
        CoverSourceKind::AppleMusic => preferred_apple_artwork_url(submitted),
        CoverSourceKind::RecordLabel
        | CoverSourceKind::ArtistWebsite
        | CoverSourceKind::Vgmdb
        | CoverSourceKind::Manual => canonicalize_cover_url(submitted),
    }
    .map_err(CoverError::InvalidUrl)
}

fn ensure_candidate_version(
    model: &cover_candidate::Model,
    expected: CoverRowVersion,
) -> Result<(), CoverError> {
    let actual = parse_row_version("cover candidate", model.candidate_id, model.row_version)?;
    if actual == expected {
        Ok(())
    } else {
        Err(CoverError::CandidateConflict {
            candidate_id: model.candidate_id,
            expected,
            actual,
        })
    }
}

fn ensure_worker_lease(
    model: &cover_candidate::Model,
    expected: CoverRowVersion,
    lease_token: Uuid,
    target: CoverCandidateState,
) -> Result<(), CoverError> {
    ensure_candidate_version(model, expected)?;
    let state = parse_candidate_state(model.candidate_id, &model.state)?;
    if state != CoverCandidateState::Fetching {
        return Err(CoverError::InvalidCandidateTransition {
            candidate_id: model.candidate_id,
            from: state,
            to: target,
        });
    }
    if model.lease_token != Some(lease_token) {
        return Err(CoverError::LeaseMismatch {
            candidate_id: model.candidate_id,
        });
    }
    let lease_is_current = model
        .lease_expires_at
        .as_ref()
        .is_some_and(|expires_at| timestamp(expires_at.to_owned()) > chrono::Utc::now());
    if !lease_is_current {
        return Err(CoverError::LeaseMismatch {
            candidate_id: model.candidate_id,
        });
    }
    Ok(())
}

fn validate_failure(failure: &CoverFetchFailure) -> Result<(), CoverError> {
    validate_failure_code(&failure.code)?;
    if failure.http_status == Some(0) {
        return Err(CoverError::InvalidInput {
            field: "http_status",
            message: "status must be positive when present",
        });
    }
    Ok(())
}

fn validate_failure_code(code: &str) -> Result<(), CoverError> {
    if code.is_empty()
        || code.len() > MAX_FAILURE_CODE_BYTES
        || !code
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        Err(CoverError::InvalidInput {
            field: "failure_code",
            message: "use 1-64 ASCII letters, digits, dot, dash, or underscore",
        })
    } else {
        Ok(())
    }
}

/// Avoid persisting URLs, credentials, or unbounded server responses. A
/// message that looks like it contains a URL is discarded entirely; the
/// bounded error code remains available to clients and metrics.
fn sanitize_message(message: &str) -> Option<String> {
    let lowercase = message.to_ascii_lowercase();
    if message.contains("://")
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
        .any(|needle| lowercase.contains(needle))
    {
        return None;
    }
    let sanitized: String = message
        .chars()
        .filter(|character| !character.is_control())
        .take(MAX_FAILURE_MESSAGE_CHARS)
        .collect();
    (!sanitized.is_empty()).then_some(sanitized)
}

fn parse_candidate_state(
    candidate_id: Uuid,
    value: &str,
) -> Result<CoverCandidateState, CoverError> {
    CoverCandidateState::from_str(value).map_err(|_| CoverError::InvalidPersistedValue {
        entity: "cover candidate",
        id: candidate_id,
        field: "state",
        value: value.to_owned(),
    })
}

fn parse_source_kind(candidate_id: Uuid, value: &str) -> Result<CoverSourceKind, CoverError> {
    CoverSourceKind::from_str(value).map_err(|_| CoverError::InvalidPersistedValue {
        entity: "cover candidate",
        id: candidate_id,
        field: "source_kind",
        value: value.to_owned(),
    })
}

fn parse_row_version(
    entity: &'static str,
    id: Uuid,
    value: i64,
) -> Result<CoverRowVersion, CoverError> {
    u64::try_from(value)
        .ok()
        .and_then(CoverRowVersion::new)
        .ok_or(CoverError::InvalidPersistedValue {
            entity,
            id,
            field: "row_version",
            value: value.to_string(),
        })
}

fn digest_from_bytes(asset_id: Uuid, bytes: Vec<u8>) -> Result<Digest, CoverError> {
    bytes
        .try_into()
        .map(Digest::new)
        .map_err(|bytes: Vec<u8>| CoverError::InvalidPersistedValue {
            entity: "cover asset",
            id: asset_id,
            field: "content_sha256",
            value: format!("{} bytes", bytes.len()),
        })
}

fn positive_i32(id: Uuid, field: &'static str, value: i32) -> Result<NonZeroU32, CoverError> {
    u32::try_from(value)
        .ok()
        .and_then(NonZeroU32::new)
        .ok_or(CoverError::InvalidPersistedValue {
            entity: "cover asset",
            id,
            field,
            value: value.to_string(),
        })
}

fn positive_i64(id: Uuid, field: &'static str, value: i64) -> Result<u64, CoverError> {
    u64::try_from(value)
        .ok()
        .filter(|value| *value > 0)
        .ok_or(CoverError::InvalidPersistedValue {
            entity: "cover asset",
            id,
            field,
            value: value.to_string(),
        })
}

fn store_disc_number(value: u16, id: Uuid) -> Result<i16, CoverError> {
    i16::try_from(value).map_err(|_| CoverError::NumericOutOfRange {
        entity: "cover scope",
        id,
        field: "disc_number",
    })
}

fn load_disc_number(id: Uuid, value: i16) -> Result<u16, CoverError> {
    u16::try_from(value).map_err(|_| CoverError::InvalidPersistedValue {
        entity: "cover scope",
        id,
        field: "disc_number",
        value: value.to_string(),
    })
}

fn missing_reference(entity: &'static str, id: Uuid, reference: &'static str) -> CoverError {
    CoverError::Database(DbErr::Custom(format!(
        "{entity} {id} references a missing {reference}"
    )))
}

#[cfg(feature = "postgres")]
type StoredTimestamp = chrono::NaiveDateTime;
#[cfg(feature = "sqlite")]
type StoredTimestamp = DateTimeUtc;

#[cfg(feature = "postgres")]
fn store_timestamp(value: DateTimeUtc) -> StoredTimestamp {
    value.naive_utc()
}

#[cfg(feature = "sqlite")]
fn store_timestamp(value: DateTimeUtc) -> StoredTimestamp {
    value
}
