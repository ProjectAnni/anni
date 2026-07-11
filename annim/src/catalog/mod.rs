//! Artist-centric collection catalog persistence and application service.
//!
//! The public snapshots intentionally omit private acquisition locators. All
//! lifecycle writes use the release row version as the aggregate lock, even
//! when the durable fact being added is a `collection_copy` row.

use std::{collections::HashMap, num::NonZeroU32, str::FromStr};

use anni_catalog::{
    AcquisitionSourceKind, AudioCodec, AudioProperties, CollectionState, QualityTier, ReleaseKind,
};
use anni_ingest::Digest;
use sea_orm::{
    prelude::{DateTimeUtc, Uuid},
    sea_query::{Expr, OnConflict},
    ActiveValue::{NotSet, Set},
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, TransactionTrait, TryInsertResult,
};
use thiserror::Error;

use crate::entities::{
    catalog_artist, catalog_release, collection_copy,
    helper::{now, timestamp},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CatalogRowVersion(u64);

impl CatalogRowVersion {
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

    fn as_i64(self, entity: &'static str, id: Uuid) -> Result<i64, CatalogError> {
        i64::try_from(self.0).map_err(|_| CatalogError::NumericOutOfRange {
            entity,
            id,
            field: "row_version",
        })
    }

    fn next(self, entity: &'static str, id: Uuid) -> Result<Self, CatalogError> {
        self.0
            .checked_add(1)
            .filter(|value| i64::try_from(*value).is_ok())
            .map(Self)
            .ok_or(CatalogError::NumericOutOfRange {
                entity,
                id,
                field: "row_version",
            })
    }
}

impl std::fmt::Display for CatalogRowVersion {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogArtistSnapshot {
    pub artist_id: Uuid,
    pub display_name: String,
    pub sort_name: Option<String>,
    pub notes: Option<String>,
    pub row_version: CatalogRowVersion,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionCopySnapshot {
    pub copy_id: Uuid,
    pub source_kind: AcquisitionSourceKind,
    pub source_label: String,
    pub codec: AudioCodec,
    pub sample_rate_hz: Option<NonZeroU32>,
    pub bit_depth: Option<u8>,
    pub channels: Option<u8>,
    pub track_count: Option<u32>,
    pub byte_length: Option<u64>,
    pub manifest_digest: Option<Digest>,
    pub quality_verified: bool,
    pub ingest_job_id: Option<Uuid>,
    pub notes: Option<String>,
    pub acquired_at: DateTimeUtc,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

impl CollectionCopySnapshot {
    pub fn audio_properties(&self) -> AudioProperties {
        AudioProperties::new(
            self.codec,
            self.sample_rate_hz,
            self.bit_depth,
            self.channels,
        )
        .expect("persisted collection copy was validated while loading")
    }

    pub fn quality_tier(&self) -> QualityTier {
        self.audio_properties().quality_tier()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogReleaseSnapshot {
    pub release_id: Uuid,
    pub artist_id: Uuid,
    pub title: String,
    pub edition: Option<String>,
    pub catalog: Option<String>,
    pub release_date: Option<String>,
    pub kind: ReleaseKind,
    pub wanted: bool,
    pub unavailable: bool,
    pub matched_album_id: Option<Uuid>,
    pub active_ingest_job_id: Option<Uuid>,
    pub notes: Option<String>,
    pub row_version: CatalogRowVersion,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
    pub copies: Vec<CollectionCopySnapshot>,
}

impl CatalogReleaseSnapshot {
    pub fn collection_state(&self) -> CollectionState {
        if self.matched_album_id.is_some() {
            CollectionState::Published
        } else if self.active_ingest_job_id.is_some() {
            CollectionState::Ingesting
        } else if !self.copies.is_empty() {
            CollectionState::Acquired
        } else if self.unavailable {
            CollectionState::Unavailable
        } else if self.wanted {
            CollectionState::Wanted
        } else {
            CollectionState::Missing
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CatalogCollectionSummary {
    pub total: u64,
    pub missing: u64,
    pub wanted: u64,
    pub acquired: u64,
    pub ingesting: u64,
    pub published: u64,
    pub unavailable: u64,
    pub collected: u64,
}

impl CatalogCollectionSummary {
    fn from_releases(releases: &[CatalogReleaseSnapshot]) -> Self {
        let mut summary = Self {
            total: releases.len() as u64,
            ..Self::default()
        };
        for state in releases
            .iter()
            .map(CatalogReleaseSnapshot::collection_state)
        {
            match state {
                CollectionState::Missing => summary.missing += 1,
                CollectionState::Wanted => summary.wanted += 1,
                CollectionState::Acquired => summary.acquired += 1,
                CollectionState::Ingesting => summary.ingesting += 1,
                CollectionState::Published => summary.published += 1,
                CollectionState::Unavailable => summary.unavailable += 1,
            }
            if state.is_collected() {
                summary.collected += 1;
            }
        }
        summary
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogArtistCollection {
    pub artist: CatalogArtistSnapshot,
    pub summary: CatalogCollectionSummary,
    pub releases: Vec<CatalogReleaseSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewCatalogArtist {
    pub artist_id: Option<Uuid>,
    pub display_name: String,
    pub sort_name: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateCatalogArtist {
    pub display_name: String,
    pub sort_name: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewCatalogRelease {
    pub release_id: Option<Uuid>,
    pub artist_id: Uuid,
    pub title: String,
    pub edition: Option<String>,
    pub catalog: Option<String>,
    pub release_date: Option<String>,
    pub kind: ReleaseKind,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateCatalogRelease {
    pub title: String,
    pub edition: Option<String>,
    pub catalog: Option<String>,
    pub release_date: Option<String>,
    pub kind: ReleaseKind,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewCollectionCopy {
    pub copy_id: Option<Uuid>,
    pub source_kind: AcquisitionSourceKind,
    pub source_label: String,
    pub private_locator: Option<String>,
    pub codec: AudioCodec,
    pub sample_rate_hz: Option<NonZeroU32>,
    pub bit_depth: Option<u8>,
    pub channels: Option<u8>,
    pub track_count: Option<u32>,
    pub byte_length: Option<u64>,
    pub manifest_digest: Option<Digest>,
    pub quality_verified: bool,
    pub ingest_job_id: Option<Uuid>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogReleaseCommand {
    MarkMissing,
    MarkWanted,
    MarkUnavailable,
    RecordCopy(NewCollectionCopy),
    BeginIngest { job_id: Uuid },
    Publish { album_id: Uuid },
    ReturnToAcquired,
}

#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("catalog artist {artist_id} already exists")]
    ArtistAlreadyExists { artist_id: Uuid },
    #[error("catalog artist {artist_id} does not exist")]
    ArtistNotFound { artist_id: Uuid },
    #[error(
        "catalog artist {artist_id} changed concurrently: expected {expected}, actual {actual}"
    )]
    ArtistConflict {
        artist_id: Uuid,
        expected: CatalogRowVersion,
        actual: CatalogRowVersion,
    },
    #[error("catalog release {release_id} already exists")]
    ReleaseAlreadyExists { release_id: Uuid },
    #[error("catalog release {release_id} does not exist")]
    ReleaseNotFound { release_id: Uuid },
    #[error(
        "catalog release {release_id} changed concurrently: expected {expected}, actual {actual}"
    )]
    ReleaseConflict {
        release_id: Uuid,
        expected: CatalogRowVersion,
        actual: CatalogRowVersion,
    },
    #[error("collection copy {copy_id} already exists")]
    CopyAlreadyExists { copy_id: Uuid },
    #[error("catalog release {release_id} cannot transition from {from} to {to}")]
    InvalidTransition {
        release_id: Uuid,
        from: CollectionState,
        to: CollectionState,
    },
    #[error("catalog release {release_id} has no acquired copy")]
    NoAcquiredCopy { release_id: Uuid },
    #[error("invalid catalog {field}: {message}")]
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
    #[error(transparent)]
    Database(#[from] DbErr),
}

#[derive(Clone)]
pub struct CatalogRepository {
    database: DatabaseConnection,
}

impl CatalogRepository {
    pub fn new(database: DatabaseConnection) -> Self {
        Self { database }
    }

    pub async fn create_artist(
        &self,
        input: NewCatalogArtist,
    ) -> Result<CatalogArtistSnapshot, CatalogError> {
        let artist_id = input.artist_id.unwrap_or_else(Uuid::new_v4);
        let result = catalog_artist::Entity::insert(catalog_artist::ActiveModel {
            id: NotSet,
            artist_id: Set(artist_id),
            display_name: Set(input.display_name),
            sort_name: Set(input.sort_name),
            notes: Set(input.notes),
            row_version: Set(1),
            created_at: NotSet,
            updated_at: NotSet,
        })
        .on_conflict(
            OnConflict::column(catalog_artist::Column::ArtistId)
                .do_nothing()
                .to_owned(),
        )
        .do_nothing()
        .exec_without_returning(&self.database)
        .await?;

        match result {
            TryInsertResult::Inserted(1) => self
                .get_artist(artist_id)
                .await?
                .ok_or(CatalogError::ArtistNotFound { artist_id }),
            TryInsertResult::Conflicted | TryInsertResult::Inserted(0) => {
                Err(CatalogError::ArtistAlreadyExists { artist_id })
            }
            TryInsertResult::Inserted(_) | TryInsertResult::Empty => Err(CatalogError::Database(
                DbErr::Custom("catalog artist insert affected an unexpected row count".to_owned()),
            )),
        }
    }

    pub async fn get_artist(
        &self,
        artist_id: Uuid,
    ) -> Result<Option<CatalogArtistSnapshot>, CatalogError> {
        catalog_artist::Entity::find()
            .filter(catalog_artist::Column::ArtistId.eq(artist_id))
            .one(&self.database)
            .await?
            .map(artist_model_to_snapshot)
            .transpose()
    }

    pub async fn list_artists(
        &self,
        search: Option<&str>,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<CatalogArtistSnapshot>, CatalogError> {
        let mut query = catalog_artist::Entity::find();
        if let Some(search) = search.filter(|value| !value.is_empty()) {
            query = query.filter(catalog_artist::Column::DisplayName.contains(search));
        }
        query
            .order_by_asc(catalog_artist::Column::SortName)
            .order_by_asc(catalog_artist::Column::DisplayName)
            .limit(limit)
            .offset(offset)
            .all(&self.database)
            .await?
            .into_iter()
            .map(artist_model_to_snapshot)
            .collect()
    }

    pub async fn create_release(
        &self,
        input: NewCatalogRelease,
    ) -> Result<CatalogReleaseSnapshot, CatalogError> {
        let artist = get_artist_model(&self.database, input.artist_id)
            .await?
            .ok_or(CatalogError::ArtistNotFound {
                artist_id: input.artist_id,
            })?;
        let release_id = input.release_id.unwrap_or_else(Uuid::new_v4);
        let result = catalog_release::Entity::insert(catalog_release::ActiveModel {
            id: NotSet,
            release_id: Set(release_id),
            artist_db_id: Set(artist.id),
            title: Set(input.title),
            edition: Set(input.edition),
            catalog: Set(input.catalog),
            release_date: Set(input.release_date),
            kind: Set(input.kind.as_str().to_owned()),
            wanted: Set(false),
            unavailable: Set(false),
            matched_album_id: Set(None),
            active_ingest_job_id: Set(None),
            notes: Set(input.notes),
            row_version: Set(1),
            created_at: NotSet,
            updated_at: NotSet,
        })
        .on_conflict(
            OnConflict::column(catalog_release::Column::ReleaseId)
                .do_nothing()
                .to_owned(),
        )
        .do_nothing()
        .exec_without_returning(&self.database)
        .await?;
        match result {
            TryInsertResult::Inserted(1) => self
                .get_release(release_id)
                .await?
                .ok_or(CatalogError::ReleaseNotFound { release_id }),
            TryInsertResult::Conflicted | TryInsertResult::Inserted(0) => {
                Err(CatalogError::ReleaseAlreadyExists { release_id })
            }
            TryInsertResult::Inserted(_) | TryInsertResult::Empty => Err(CatalogError::Database(
                DbErr::Custom("catalog release insert affected an unexpected row count".to_owned()),
            )),
        }
    }

    pub async fn get_release(
        &self,
        release_id: Uuid,
    ) -> Result<Option<CatalogReleaseSnapshot>, CatalogError> {
        let Some(model) = get_release_model(&self.database, release_id).await? else {
            return Ok(None);
        };
        let artist = catalog_artist::Entity::find_by_id(model.artist_db_id)
            .one(&self.database)
            .await?
            .ok_or_else(|| {
                CatalogError::Database(DbErr::Custom(format!(
                    "catalog release {release_id} references a missing artist"
                )))
            })?;
        let copies = collection_copy::Entity::find()
            .filter(collection_copy::Column::ReleaseDbId.eq(model.id))
            .order_by_desc(collection_copy::Column::AcquiredAt)
            .all(&self.database)
            .await?;
        release_model_to_snapshot(model, artist.artist_id, copies).map(Some)
    }

    pub async fn get_artist_collection(
        &self,
        artist_id: Uuid,
    ) -> Result<Option<CatalogArtistCollection>, CatalogError> {
        let Some(artist_model) = get_artist_model(&self.database, artist_id).await? else {
            return Ok(None);
        };
        let release_models = catalog_release::Entity::find()
            .filter(catalog_release::Column::ArtistDbId.eq(artist_model.id))
            .order_by_asc(catalog_release::Column::ReleaseDate)
            .order_by_asc(catalog_release::Column::Id)
            .all(&self.database)
            .await?;
        let release_db_ids: Vec<_> = release_models.iter().map(|model| model.id).collect();
        let mut copies_by_release: HashMap<i32, Vec<collection_copy::Model>> = HashMap::new();
        if !release_db_ids.is_empty() {
            for copy in collection_copy::Entity::find()
                .filter(collection_copy::Column::ReleaseDbId.is_in(release_db_ids))
                .order_by_desc(collection_copy::Column::AcquiredAt)
                .all(&self.database)
                .await?
            {
                copies_by_release
                    .entry(copy.release_db_id)
                    .or_default()
                    .push(copy);
            }
        }
        let releases = release_models
            .into_iter()
            .map(|model| {
                let copies = copies_by_release.remove(&model.id).unwrap_or_default();
                release_model_to_snapshot(model, artist_id, copies)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let summary = CatalogCollectionSummary::from_releases(&releases);
        Ok(Some(CatalogArtistCollection {
            artist: artist_model_to_snapshot(artist_model)?,
            summary,
            releases,
        }))
    }

    pub async fn update_artist(
        &self,
        artist_id: Uuid,
        expected: CatalogRowVersion,
        input: UpdateCatalogArtist,
    ) -> Result<CatalogArtistSnapshot, CatalogError> {
        require_non_empty("display_name", &input.display_name)?;
        let next = expected.next("artist", artist_id)?;
        let result = catalog_artist::Entity::update_many()
            .col_expr(
                catalog_artist::Column::DisplayName,
                Expr::value(input.display_name),
            )
            .col_expr(
                catalog_artist::Column::SortName,
                Expr::value(input.sort_name),
            )
            .col_expr(catalog_artist::Column::Notes, Expr::value(input.notes))
            .col_expr(
                catalog_artist::Column::RowVersion,
                Expr::value(next.as_i64("artist", artist_id)?),
            )
            .col_expr(
                catalog_artist::Column::UpdatedAt,
                Expr::current_timestamp().into(),
            )
            .filter(catalog_artist::Column::ArtistId.eq(artist_id))
            .filter(catalog_artist::Column::RowVersion.eq(expected.as_i64("artist", artist_id)?))
            .exec(&self.database)
            .await?;
        if result.rows_affected == 1 {
            return self
                .get_artist(artist_id)
                .await?
                .ok_or(CatalogError::ArtistNotFound { artist_id });
        }
        match self.get_artist(artist_id).await? {
            Some(actual) => Err(CatalogError::ArtistConflict {
                artist_id,
                expected,
                actual: actual.row_version,
            }),
            None => Err(CatalogError::ArtistNotFound { artist_id }),
        }
    }

    pub async fn update_release(
        &self,
        release_id: Uuid,
        expected: CatalogRowVersion,
        input: UpdateCatalogRelease,
    ) -> Result<CatalogReleaseSnapshot, CatalogError> {
        require_non_empty("title", &input.title)?;
        let next = expected.next("release", release_id)?;
        let result = catalog_release::Entity::update_many()
            .col_expr(catalog_release::Column::Title, Expr::value(input.title))
            .col_expr(catalog_release::Column::Edition, Expr::value(input.edition))
            .col_expr(catalog_release::Column::Catalog, Expr::value(input.catalog))
            .col_expr(
                catalog_release::Column::ReleaseDate,
                Expr::value(input.release_date),
            )
            .col_expr(
                catalog_release::Column::Kind,
                Expr::value(input.kind.as_str()),
            )
            .col_expr(catalog_release::Column::Notes, Expr::value(input.notes))
            .col_expr(
                catalog_release::Column::RowVersion,
                Expr::value(next.as_i64("release", release_id)?),
            )
            .col_expr(
                catalog_release::Column::UpdatedAt,
                Expr::current_timestamp().into(),
            )
            .filter(catalog_release::Column::ReleaseId.eq(release_id))
            .filter(catalog_release::Column::RowVersion.eq(expected.as_i64("release", release_id)?))
            .exec(&self.database)
            .await?;
        if result.rows_affected == 1 {
            return self
                .get_release(release_id)
                .await?
                .ok_or(CatalogError::ReleaseNotFound { release_id });
        }
        self.release_conflict_or_not_found(release_id, expected)
            .await
    }

    async fn set_release_lifecycle(
        &self,
        release_id: Uuid,
        expected: CatalogRowVersion,
        wanted: bool,
        unavailable: bool,
        matched_album_id: Option<Uuid>,
        active_ingest_job_id: Option<Uuid>,
    ) -> Result<CatalogReleaseSnapshot, CatalogError> {
        cas_release_lifecycle(
            &self.database,
            release_id,
            expected,
            wanted,
            unavailable,
            matched_album_id,
            active_ingest_job_id,
        )
        .await?;
        self.get_release(release_id)
            .await?
            .ok_or(CatalogError::ReleaseNotFound { release_id })
    }

    async fn record_copy(
        &self,
        release_id: Uuid,
        expected: CatalogRowVersion,
        input: NewCollectionCopy,
    ) -> Result<CatalogReleaseSnapshot, CatalogError> {
        require_non_empty("source_label", &input.source_label)?;
        AudioProperties::new(
            input.codec,
            input.sample_rate_hz,
            input.bit_depth,
            input.channels,
        )
        .map_err(|_| CatalogError::InvalidInput {
            field: "audio_properties",
            message: "bit depth and channel count must be positive when present",
        })?;
        if input.track_count == Some(0) {
            return Err(CatalogError::InvalidInput {
                field: "track_count",
                message: "track count must be positive when present",
            });
        }
        if input.byte_length == Some(0) {
            return Err(CatalogError::InvalidInput {
                field: "byte_length",
                message: "byte length must be positive when present",
            });
        }
        let copy_id = input.copy_id.unwrap_or_else(Uuid::new_v4);
        let transaction = self.database.begin().await?;
        cas_release_lifecycle(&transaction, release_id, expected, false, false, None, None).await?;
        let release = get_release_model(&transaction, release_id)
            .await?
            .ok_or(CatalogError::ReleaseNotFound { release_id })?;
        let active_model = collection_copy::ActiveModel {
            id: NotSet,
            copy_id: Set(copy_id),
            release_db_id: Set(release.id),
            source_kind: Set(input.source_kind.as_str().to_owned()),
            source_label: Set(input.source_label),
            private_locator: Set(input.private_locator),
            codec: Set(input.codec.as_str().to_owned()),
            sample_rate_hz: Set(optional_u32_to_i32(
                copy_id,
                "sample_rate_hz",
                input.sample_rate_hz.map(NonZeroU32::get),
            )?),
            bit_depth: Set(input.bit_depth.map(i16::from)),
            channels: Set(input.channels.map(i16::from)),
            track_count: Set(optional_u32_to_i32(
                copy_id,
                "track_count",
                input.track_count,
            )?),
            byte_length: Set(optional_u64_to_i64(
                copy_id,
                "byte_length",
                input.byte_length,
            )?),
            manifest_digest: Set(input
                .manifest_digest
                .map(|digest| digest.as_bytes().to_vec())),
            quality_verified: Set(input.quality_verified),
            ingest_job_id: Set(input.ingest_job_id),
            notes: Set(input.notes),
            acquired_at: Set(now()),
            created_at: NotSet,
            updated_at: NotSet,
        };
        let result = collection_copy::Entity::insert(active_model)
            .on_conflict(
                OnConflict::column(collection_copy::Column::CopyId)
                    .do_nothing()
                    .to_owned(),
            )
            .do_nothing()
            .exec_without_returning(&transaction)
            .await?;
        match result {
            TryInsertResult::Inserted(1) => transaction.commit().await?,
            TryInsertResult::Conflicted | TryInsertResult::Inserted(0) => {
                return Err(CatalogError::CopyAlreadyExists { copy_id });
            }
            TryInsertResult::Inserted(_) | TryInsertResult::Empty => {
                return Err(CatalogError::Database(DbErr::Custom(
                    "collection copy insert affected an unexpected row count".to_owned(),
                )));
            }
        }
        self.get_release(release_id)
            .await?
            .ok_or(CatalogError::ReleaseNotFound { release_id })
    }

    async fn release_conflict_or_not_found(
        &self,
        release_id: Uuid,
        expected: CatalogRowVersion,
    ) -> Result<CatalogReleaseSnapshot, CatalogError> {
        match self.get_release(release_id).await? {
            Some(actual) => Err(CatalogError::ReleaseConflict {
                release_id,
                expected,
                actual: actual.row_version,
            }),
            None => Err(CatalogError::ReleaseNotFound { release_id }),
        }
    }
}

#[derive(Clone)]
pub struct CatalogService {
    repository: CatalogRepository,
}

impl CatalogService {
    pub fn new(repository: CatalogRepository) -> Self {
        Self { repository }
    }

    pub const fn repository(&self) -> &CatalogRepository {
        &self.repository
    }

    pub async fn create_artist(
        &self,
        input: NewCatalogArtist,
    ) -> Result<CatalogArtistSnapshot, CatalogError> {
        require_non_empty("display_name", &input.display_name)?;
        self.repository.create_artist(input).await
    }

    pub async fn update_artist(
        &self,
        artist_id: Uuid,
        expected: CatalogRowVersion,
        input: UpdateCatalogArtist,
    ) -> Result<CatalogArtistSnapshot, CatalogError> {
        self.repository
            .update_artist(artist_id, expected, input)
            .await
    }

    pub async fn list_artists(
        &self,
        search: Option<&str>,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<CatalogArtistSnapshot>, CatalogError> {
        self.repository.list_artists(search, limit, offset).await
    }

    pub async fn create_release(
        &self,
        input: NewCatalogRelease,
    ) -> Result<CatalogReleaseSnapshot, CatalogError> {
        require_non_empty("title", &input.title)?;
        self.repository.create_release(input).await
    }

    pub async fn update_release(
        &self,
        release_id: Uuid,
        expected: CatalogRowVersion,
        input: UpdateCatalogRelease,
    ) -> Result<CatalogReleaseSnapshot, CatalogError> {
        self.repository
            .update_release(release_id, expected, input)
            .await
    }

    pub async fn artist_collection(
        &self,
        artist_id: Uuid,
        state: Option<CollectionState>,
    ) -> Result<Option<CatalogArtistCollection>, CatalogError> {
        let Some(mut collection) = self.repository.get_artist_collection(artist_id).await? else {
            return Ok(None);
        };
        if let Some(state) = state {
            collection
                .releases
                .retain(|release| release.collection_state() == state);
        }
        collection.summary = CatalogCollectionSummary::from_releases(&collection.releases);
        Ok(Some(collection))
    }

    pub async fn execute_release_command(
        &self,
        release_id: Uuid,
        expected: CatalogRowVersion,
        command: CatalogReleaseCommand,
    ) -> Result<CatalogReleaseSnapshot, CatalogError> {
        let current = self
            .repository
            .get_release(release_id)
            .await?
            .ok_or(CatalogError::ReleaseNotFound { release_id })?;
        if current.row_version != expected {
            return Err(CatalogError::ReleaseConflict {
                release_id,
                expected,
                actual: current.row_version,
            });
        }
        let state = current.collection_state();
        match command {
            CatalogReleaseCommand::MarkMissing => {
                ensure_marker_transition(release_id, state, CollectionState::Missing)?;
                self.repository
                    .set_release_lifecycle(release_id, expected, false, false, None, None)
                    .await
            }
            CatalogReleaseCommand::MarkWanted => {
                ensure_marker_transition(release_id, state, CollectionState::Wanted)?;
                self.repository
                    .set_release_lifecycle(release_id, expected, true, false, None, None)
                    .await
            }
            CatalogReleaseCommand::MarkUnavailable => {
                ensure_marker_transition(release_id, state, CollectionState::Unavailable)?;
                self.repository
                    .set_release_lifecycle(release_id, expected, false, true, None, None)
                    .await
            }
            CatalogReleaseCommand::RecordCopy(copy) => {
                ensure_record_copy_transition(release_id, state)?;
                self.repository
                    .record_copy(release_id, expected, copy)
                    .await
            }
            CatalogReleaseCommand::BeginIngest { job_id } => {
                ensure_exact_transition(
                    release_id,
                    state,
                    CollectionState::Acquired,
                    CollectionState::Ingesting,
                )?;
                self.repository
                    .set_release_lifecycle(release_id, expected, false, false, None, Some(job_id))
                    .await
            }
            CatalogReleaseCommand::Publish { album_id } => {
                ensure_exact_transition(
                    release_id,
                    state,
                    CollectionState::Ingesting,
                    CollectionState::Published,
                )?;
                self.repository
                    .set_release_lifecycle(release_id, expected, false, false, Some(album_id), None)
                    .await
            }
            CatalogReleaseCommand::ReturnToAcquired => {
                if current.copies.is_empty() {
                    return Err(CatalogError::NoAcquiredCopy { release_id });
                }
                ensure_transition(release_id, state, CollectionState::Acquired)?;
                self.repository
                    .set_release_lifecycle(release_id, expected, false, false, None, None)
                    .await
            }
        }
    }
}

async fn get_artist_model<C: ConnectionTrait>(
    connection: &C,
    artist_id: Uuid,
) -> Result<Option<catalog_artist::Model>, CatalogError> {
    Ok(catalog_artist::Entity::find()
        .filter(catalog_artist::Column::ArtistId.eq(artist_id))
        .one(connection)
        .await?)
}

async fn get_release_model<C: ConnectionTrait>(
    connection: &C,
    release_id: Uuid,
) -> Result<Option<catalog_release::Model>, CatalogError> {
    Ok(catalog_release::Entity::find()
        .filter(catalog_release::Column::ReleaseId.eq(release_id))
        .one(connection)
        .await?)
}

#[allow(clippy::too_many_arguments)]
async fn cas_release_lifecycle<C: ConnectionTrait>(
    connection: &C,
    release_id: Uuid,
    expected: CatalogRowVersion,
    wanted: bool,
    unavailable: bool,
    matched_album_id: Option<Uuid>,
    active_ingest_job_id: Option<Uuid>,
) -> Result<CatalogRowVersion, CatalogError> {
    let next = expected.next("release", release_id)?;
    let result = catalog_release::Entity::update_many()
        .col_expr(catalog_release::Column::Wanted, Expr::value(wanted))
        .col_expr(
            catalog_release::Column::Unavailable,
            Expr::value(unavailable),
        )
        .col_expr(
            catalog_release::Column::MatchedAlbumId,
            Expr::value(matched_album_id),
        )
        .col_expr(
            catalog_release::Column::ActiveIngestJobId,
            Expr::value(active_ingest_job_id),
        )
        .col_expr(
            catalog_release::Column::RowVersion,
            Expr::value(next.as_i64("release", release_id)?),
        )
        .col_expr(
            catalog_release::Column::UpdatedAt,
            Expr::current_timestamp().into(),
        )
        .filter(catalog_release::Column::ReleaseId.eq(release_id))
        .filter(catalog_release::Column::RowVersion.eq(expected.as_i64("release", release_id)?))
        .exec(connection)
        .await?;
    if result.rows_affected == 1 {
        return Ok(next);
    }
    match get_release_model(connection, release_id).await? {
        Some(actual) => Err(CatalogError::ReleaseConflict {
            release_id,
            expected,
            actual: parse_row_version("release", release_id, actual.row_version)?,
        }),
        None => Err(CatalogError::ReleaseNotFound { release_id }),
    }
}

fn artist_model_to_snapshot(
    model: catalog_artist::Model,
) -> Result<CatalogArtistSnapshot, CatalogError> {
    let artist_id = model.artist_id;
    Ok(CatalogArtistSnapshot {
        artist_id,
        display_name: model.display_name,
        sort_name: model.sort_name,
        notes: model.notes,
        row_version: parse_row_version("artist", artist_id, model.row_version)?,
        created_at: timestamp(model.created_at),
        updated_at: timestamp(model.updated_at),
    })
}

fn release_model_to_snapshot(
    model: catalog_release::Model,
    artist_id: Uuid,
    copies: Vec<collection_copy::Model>,
) -> Result<CatalogReleaseSnapshot, CatalogError> {
    let release_id = model.release_id;
    let kind =
        ReleaseKind::from_str(&model.kind).map_err(|_| CatalogError::InvalidPersistedValue {
            entity: "release",
            id: release_id,
            field: "kind",
            value: model.kind.clone(),
        })?;
    Ok(CatalogReleaseSnapshot {
        release_id,
        artist_id,
        title: model.title,
        edition: model.edition,
        catalog: model.catalog,
        release_date: model.release_date,
        kind,
        wanted: model.wanted,
        unavailable: model.unavailable,
        matched_album_id: model.matched_album_id,
        active_ingest_job_id: model.active_ingest_job_id,
        notes: model.notes,
        row_version: parse_row_version("release", release_id, model.row_version)?,
        created_at: timestamp(model.created_at),
        updated_at: timestamp(model.updated_at),
        copies: copies
            .into_iter()
            .map(|copy| copy_model_to_snapshot(release_id, copy))
            .collect::<Result<_, _>>()?,
    })
}

fn copy_model_to_snapshot(
    _release_id: Uuid,
    model: collection_copy::Model,
) -> Result<CollectionCopySnapshot, CatalogError> {
    let copy_id = model.copy_id;
    let invalid = |field: &'static str, value: String| CatalogError::InvalidPersistedValue {
        entity: "collection copy",
        id: copy_id,
        field,
        value,
    };
    let source_kind = AcquisitionSourceKind::from_str(&model.source_kind)
        .map_err(|_| invalid("source_kind", model.source_kind.clone()))?;
    let codec =
        AudioCodec::from_str(&model.codec).map_err(|_| invalid("codec", model.codec.clone()))?;
    let sample_rate_hz = positive_i32_to_nonzero(model.sample_rate_hz)
        .ok_or_else(|| invalid("sample_rate_hz", format!("{:?}", model.sample_rate_hz)))?;
    let bit_depth = positive_i16_to_u8(model.bit_depth)
        .ok_or_else(|| invalid("bit_depth", format!("{:?}", model.bit_depth)))?;
    let channels = positive_i16_to_u8(model.channels)
        .ok_or_else(|| invalid("channels", format!("{:?}", model.channels)))?;
    AudioProperties::new(codec, sample_rate_hz, bit_depth, channels)
        .map_err(|error| invalid("audio_properties", error.to_string()))?;
    let track_count = positive_i32_to_u32(model.track_count)
        .ok_or_else(|| invalid("track_count", format!("{:?}", model.track_count)))?;
    let byte_length = positive_i64_to_u64(model.byte_length)
        .ok_or_else(|| invalid("byte_length", format!("{:?}", model.byte_length)))?;
    let manifest_digest = model
        .manifest_digest
        .map(|bytes| digest_from_bytes(copy_id, bytes))
        .transpose()?;
    Ok(CollectionCopySnapshot {
        copy_id,
        source_kind,
        source_label: model.source_label,
        codec,
        sample_rate_hz,
        bit_depth,
        channels,
        track_count,
        byte_length,
        manifest_digest,
        quality_verified: model.quality_verified,
        ingest_job_id: model.ingest_job_id,
        notes: model.notes,
        acquired_at: timestamp(model.acquired_at),
        created_at: timestamp(model.created_at),
        updated_at: timestamp(model.updated_at),
    })
}

fn parse_row_version(
    entity: &'static str,
    id: Uuid,
    value: i64,
) -> Result<CatalogRowVersion, CatalogError> {
    u64::try_from(value)
        .ok()
        .and_then(CatalogRowVersion::new)
        .ok_or(CatalogError::InvalidPersistedValue {
            entity,
            id,
            field: "row_version",
            value: value.to_string(),
        })
}

fn positive_i32_to_nonzero(value: Option<i32>) -> Option<Option<NonZeroU32>> {
    match value {
        None => Some(None),
        Some(value) => u32::try_from(value)
            .ok()
            .and_then(NonZeroU32::new)
            .map(Some),
    }
}

fn positive_i16_to_u8(value: Option<i16>) -> Option<Option<u8>> {
    match value {
        None => Some(None),
        Some(value) => u8::try_from(value)
            .ok()
            .filter(|value| *value > 0)
            .map(Some),
    }
}

fn positive_i32_to_u32(value: Option<i32>) -> Option<Option<u32>> {
    match value {
        None => Some(None),
        Some(value) => u32::try_from(value)
            .ok()
            .filter(|value| *value > 0)
            .map(Some),
    }
}

fn positive_i64_to_u64(value: Option<i64>) -> Option<Option<u64>> {
    match value {
        None => Some(None),
        Some(value) => u64::try_from(value)
            .ok()
            .filter(|value| *value > 0)
            .map(Some),
    }
}

fn digest_from_bytes(copy_id: Uuid, bytes: Vec<u8>) -> Result<Digest, CatalogError> {
    let value = bytes
        .try_into()
        .map_err(|bytes: Vec<u8>| CatalogError::InvalidPersistedValue {
            entity: "collection copy",
            id: copy_id,
            field: "manifest_digest",
            value: format!("{} bytes", bytes.len()),
        })?;
    Ok(Digest::new(value))
}

fn optional_u32_to_i32(
    id: Uuid,
    field: &'static str,
    value: Option<u32>,
) -> Result<Option<i32>, CatalogError> {
    value
        .map(|value| {
            i32::try_from(value).map_err(|_| CatalogError::NumericOutOfRange {
                entity: "collection copy",
                id,
                field,
            })
        })
        .transpose()
}

fn optional_u64_to_i64(
    id: Uuid,
    field: &'static str,
    value: Option<u64>,
) -> Result<Option<i64>, CatalogError> {
    value
        .map(|value| {
            i64::try_from(value).map_err(|_| CatalogError::NumericOutOfRange {
                entity: "collection copy",
                id,
                field,
            })
        })
        .transpose()
}

fn require_non_empty(field: &'static str, value: &str) -> Result<(), CatalogError> {
    if value.is_empty() {
        Err(CatalogError::InvalidInput {
            field,
            message: "value must not be empty",
        })
    } else {
        Ok(())
    }
}

fn ensure_marker_transition(
    release_id: Uuid,
    from: CollectionState,
    to: CollectionState,
) -> Result<(), CatalogError> {
    if matches!(
        from,
        CollectionState::Missing | CollectionState::Wanted | CollectionState::Unavailable
    ) && from.can_transition_to(to)
    {
        Ok(())
    } else {
        Err(CatalogError::InvalidTransition {
            release_id,
            from,
            to,
        })
    }
}

fn ensure_transition(
    release_id: Uuid,
    from: CollectionState,
    to: CollectionState,
) -> Result<(), CatalogError> {
    if from.can_transition_to(to) {
        Ok(())
    } else {
        Err(CatalogError::InvalidTransition {
            release_id,
            from,
            to,
        })
    }
}

fn ensure_record_copy_transition(
    release_id: Uuid,
    from: CollectionState,
) -> Result<(), CatalogError> {
    if matches!(
        from,
        CollectionState::Missing
            | CollectionState::Wanted
            | CollectionState::Unavailable
            | CollectionState::Acquired
    ) {
        Ok(())
    } else {
        Err(CatalogError::InvalidTransition {
            release_id,
            from,
            to: CollectionState::Acquired,
        })
    }
}

fn ensure_exact_transition(
    release_id: Uuid,
    from: CollectionState,
    required: CollectionState,
    to: CollectionState,
) -> Result<(), CatalogError> {
    if from == required && from.can_transition_to(to) {
        Ok(())
    } else {
        Err(CatalogError::InvalidTransition {
            release_id,
            from,
            to,
        })
    }
}
