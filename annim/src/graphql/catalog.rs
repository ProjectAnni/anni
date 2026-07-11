use std::num::NonZeroU32;

use anni_catalog::{AcquisitionSourceKind, AudioCodec, CollectionState, QualityTier, ReleaseKind};
use anni_ingest::Digest;
use async_graphql::{
    Context, Enum, Error, ErrorExtensions, InputObject, OneofObject, Result, SimpleObject,
};
use chrono::{DateTime, Utc};
use sea_orm::prelude::Uuid;

use crate::catalog::{
    CatalogArtistCollection, CatalogArtistSnapshot, CatalogCollectionSummary, CatalogError,
    CatalogReleaseCommand, CatalogReleaseSnapshot, CatalogRowVersion, CatalogService,
    CollectionCopySnapshot, NewCatalogArtist, NewCatalogRelease, NewCollectionCopy,
    UpdateCatalogArtist, UpdateCatalogRelease,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum CatalogCollectionState {
    Missing,
    Wanted,
    Acquired,
    Ingesting,
    Published,
    Unavailable,
}

impl From<CollectionState> for CatalogCollectionState {
    fn from(value: CollectionState) -> Self {
        match value {
            CollectionState::Missing => Self::Missing,
            CollectionState::Wanted => Self::Wanted,
            CollectionState::Acquired => Self::Acquired,
            CollectionState::Ingesting => Self::Ingesting,
            CollectionState::Published => Self::Published,
            CollectionState::Unavailable => Self::Unavailable,
        }
    }
}

impl From<CatalogCollectionState> for CollectionState {
    fn from(value: CatalogCollectionState) -> Self {
        match value {
            CatalogCollectionState::Missing => Self::Missing,
            CatalogCollectionState::Wanted => Self::Wanted,
            CatalogCollectionState::Acquired => Self::Acquired,
            CatalogCollectionState::Ingesting => Self::Ingesting,
            CatalogCollectionState::Published => Self::Published,
            CatalogCollectionState::Unavailable => Self::Unavailable,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum CatalogReleaseKind {
    Album,
    Single,
    Ep,
    Soundtrack,
    Compilation,
    Other,
}

impl From<ReleaseKind> for CatalogReleaseKind {
    fn from(value: ReleaseKind) -> Self {
        match value {
            ReleaseKind::Album => Self::Album,
            ReleaseKind::Single => Self::Single,
            ReleaseKind::Ep => Self::Ep,
            ReleaseKind::Soundtrack => Self::Soundtrack,
            ReleaseKind::Compilation => Self::Compilation,
            ReleaseKind::Other => Self::Other,
        }
    }
}

impl From<CatalogReleaseKind> for ReleaseKind {
    fn from(value: CatalogReleaseKind) -> Self {
        match value {
            CatalogReleaseKind::Album => Self::Album,
            CatalogReleaseKind::Single => Self::Single,
            CatalogReleaseKind::Ep => Self::Ep,
            CatalogReleaseKind::Soundtrack => Self::Soundtrack,
            CatalogReleaseKind::Compilation => Self::Compilation,
            CatalogReleaseKind::Other => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum CatalogAcquisitionSourceKind {
    OwnedCd,
    AngelAnime,
    PrivateTracker,
    BitTorrent,
    FriendShare,
    Streaming,
    Other,
}

impl From<AcquisitionSourceKind> for CatalogAcquisitionSourceKind {
    fn from(value: AcquisitionSourceKind) -> Self {
        match value {
            AcquisitionSourceKind::OwnedCd => Self::OwnedCd,
            AcquisitionSourceKind::AngelAnime => Self::AngelAnime,
            AcquisitionSourceKind::PrivateTracker => Self::PrivateTracker,
            AcquisitionSourceKind::BitTorrent => Self::BitTorrent,
            AcquisitionSourceKind::FriendShare => Self::FriendShare,
            AcquisitionSourceKind::Streaming => Self::Streaming,
            AcquisitionSourceKind::Other => Self::Other,
        }
    }
}

impl From<CatalogAcquisitionSourceKind> for AcquisitionSourceKind {
    fn from(value: CatalogAcquisitionSourceKind) -> Self {
        match value {
            CatalogAcquisitionSourceKind::OwnedCd => Self::OwnedCd,
            CatalogAcquisitionSourceKind::AngelAnime => Self::AngelAnime,
            CatalogAcquisitionSourceKind::PrivateTracker => Self::PrivateTracker,
            CatalogAcquisitionSourceKind::BitTorrent => Self::BitTorrent,
            CatalogAcquisitionSourceKind::FriendShare => Self::FriendShare,
            CatalogAcquisitionSourceKind::Streaming => Self::Streaming,
            CatalogAcquisitionSourceKind::Other => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum CatalogAudioCodec {
    Flac,
    Wav,
    Alac,
    Aac,
    Mp3,
    Opus,
    Other,
}

impl From<AudioCodec> for CatalogAudioCodec {
    fn from(value: AudioCodec) -> Self {
        match value {
            AudioCodec::Flac => Self::Flac,
            AudioCodec::Wav => Self::Wav,
            AudioCodec::Alac => Self::Alac,
            AudioCodec::Aac => Self::Aac,
            AudioCodec::Mp3 => Self::Mp3,
            AudioCodec::Opus => Self::Opus,
            AudioCodec::Other => Self::Other,
        }
    }
}

impl From<CatalogAudioCodec> for AudioCodec {
    fn from(value: CatalogAudioCodec) -> Self {
        match value {
            CatalogAudioCodec::Flac => Self::Flac,
            CatalogAudioCodec::Wav => Self::Wav,
            CatalogAudioCodec::Alac => Self::Alac,
            CatalogAudioCodec::Aac => Self::Aac,
            CatalogAudioCodec::Mp3 => Self::Mp3,
            CatalogAudioCodec::Opus => Self::Opus,
            CatalogAudioCodec::Other => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum CatalogAudioQualityTier {
    Unknown,
    Lossy,
    Lossless,
    HiResLossless,
}

impl From<QualityTier> for CatalogAudioQualityTier {
    fn from(value: QualityTier) -> Self {
        match value {
            QualityTier::Unknown => Self::Unknown,
            QualityTier::Lossy => Self::Lossy,
            QualityTier::Lossless => Self::Lossless,
            QualityTier::HiResLossless => Self::HiResLossless,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, SimpleObject)]
pub struct CatalogArtistInfo {
    artist_id: Uuid,
    display_name: String,
    sort_name: Option<String>,
    notes: Option<String>,
    row_version: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<CatalogArtistSnapshot> for CatalogArtistInfo {
    fn from(value: CatalogArtistSnapshot) -> Self {
        Self {
            artist_id: value.artist_id,
            display_name: value.display_name,
            sort_name: value.sort_name,
            notes: value.notes,
            row_version: value.row_version.to_string(),
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, SimpleObject)]
pub struct CollectionCopyInfo {
    copy_id: Uuid,
    source_kind: CatalogAcquisitionSourceKind,
    source_label: String,
    codec: CatalogAudioCodec,
    quality_tier: CatalogAudioQualityTier,
    sample_rate_hz: Option<i32>,
    bit_depth: Option<i32>,
    channels: Option<i32>,
    track_count: Option<i32>,
    byte_length: Option<String>,
    manifest_digest: Option<String>,
    quality_verified: bool,
    ingest_job_id: Option<Uuid>,
    notes: Option<String>,
    acquired_at: DateTime<Utc>,
}

impl From<CollectionCopySnapshot> for CollectionCopyInfo {
    fn from(value: CollectionCopySnapshot) -> Self {
        let quality_tier = value.quality_tier().into();
        Self {
            copy_id: value.copy_id,
            source_kind: value.source_kind.into(),
            source_label: value.source_label,
            codec: value.codec.into(),
            quality_tier,
            sample_rate_hz: value.sample_rate_hz.map(|value| value.get() as i32),
            bit_depth: value.bit_depth.map(i32::from),
            channels: value.channels.map(i32::from),
            track_count: value.track_count.map(|value| value as i32),
            byte_length: value.byte_length.map(|value| value.to_string()),
            manifest_digest: value.manifest_digest.map(|value| value.to_string()),
            quality_verified: value.quality_verified,
            ingest_job_id: value.ingest_job_id,
            notes: value.notes,
            acquired_at: value.acquired_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, SimpleObject)]
pub struct CatalogReleaseInfo {
    release_id: Uuid,
    artist_id: Uuid,
    title: String,
    edition: Option<String>,
    catalog: Option<String>,
    release_date: Option<String>,
    kind: CatalogReleaseKind,
    collection_state: CatalogCollectionState,
    wanted: bool,
    unavailable: bool,
    matched_album_id: Option<Uuid>,
    active_ingest_job_id: Option<Uuid>,
    notes: Option<String>,
    row_version: String,
    copies: Vec<CollectionCopyInfo>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<CatalogReleaseSnapshot> for CatalogReleaseInfo {
    fn from(value: CatalogReleaseSnapshot) -> Self {
        let collection_state = value.collection_state().into();
        Self {
            release_id: value.release_id,
            artist_id: value.artist_id,
            title: value.title,
            edition: value.edition,
            catalog: value.catalog,
            release_date: value.release_date,
            kind: value.kind.into(),
            collection_state,
            wanted: value.wanted,
            unavailable: value.unavailable,
            matched_album_id: value.matched_album_id,
            active_ingest_job_id: value.active_ingest_job_id,
            notes: value.notes,
            row_version: value.row_version.to_string(),
            copies: value.copies.into_iter().map(Into::into).collect(),
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, SimpleObject)]
pub struct CatalogCollectionSummaryInfo {
    total: String,
    missing: String,
    wanted: String,
    acquired: String,
    ingesting: String,
    published: String,
    unavailable: String,
    collected: String,
}

impl From<CatalogCollectionSummary> for CatalogCollectionSummaryInfo {
    fn from(value: CatalogCollectionSummary) -> Self {
        Self {
            total: value.total.to_string(),
            missing: value.missing.to_string(),
            wanted: value.wanted.to_string(),
            acquired: value.acquired.to_string(),
            ingesting: value.ingesting.to_string(),
            published: value.published.to_string(),
            unavailable: value.unavailable.to_string(),
            collected: value.collected.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, SimpleObject)]
pub struct CatalogArtistCollectionInfo {
    artist: CatalogArtistInfo,
    summary: CatalogCollectionSummaryInfo,
    release_total_count: String,
    releases: Vec<CatalogReleaseInfo>,
}

impl CatalogArtistCollectionInfo {
    fn from_snapshot(value: CatalogArtistCollection, limit: usize, offset: usize) -> Self {
        let release_total_count = value.releases.len();
        Self {
            artist: value.artist.into(),
            summary: value.summary.into(),
            release_total_count: release_total_count.to_string(),
            releases: value
                .releases
                .into_iter()
                .skip(offset)
                .take(limit)
                .map(Into::into)
                .collect(),
        }
    }
}

#[derive(Debug, InputObject)]
pub struct CreateCatalogArtistInput {
    artist_id: Option<Uuid>,
    display_name: String,
    sort_name: Option<String>,
    notes: Option<String>,
}

#[derive(Debug, InputObject)]
pub struct UpdateCatalogArtistInput {
    artist_id: Uuid,
    expected_row_version: String,
    display_name: String,
    sort_name: Option<String>,
    notes: Option<String>,
}

#[derive(Debug, InputObject)]
pub struct CreateCatalogReleaseInput {
    release_id: Option<Uuid>,
    artist_id: Uuid,
    title: String,
    edition: Option<String>,
    catalog: Option<String>,
    release_date: Option<String>,
    kind: CatalogReleaseKind,
    notes: Option<String>,
}

#[derive(Debug, InputObject)]
pub struct UpdateCatalogReleaseInput {
    release_id: Uuid,
    expected_row_version: String,
    title: String,
    edition: Option<String>,
    catalog: Option<String>,
    release_date: Option<String>,
    kind: CatalogReleaseKind,
    notes: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum CatalogCommandSignal {
    Execute,
}

#[derive(Debug, InputObject)]
pub struct RecordCollectionCopyInput {
    copy_id: Option<Uuid>,
    source_kind: CatalogAcquisitionSourceKind,
    source_label: String,
    private_locator: Option<String>,
    codec: CatalogAudioCodec,
    sample_rate_hz: Option<i32>,
    bit_depth: Option<i32>,
    channels: Option<i32>,
    track_count: Option<i32>,
    byte_length: Option<String>,
    manifest_digest: Option<String>,
    #[graphql(default = false)]
    quality_verified: bool,
    ingest_job_id: Option<Uuid>,
    notes: Option<String>,
}

#[derive(Debug, InputObject)]
pub struct BeginCatalogIngestInput {
    job_id: Uuid,
}

#[derive(Debug, InputObject)]
pub struct PublishCatalogReleaseInput {
    album_id: Uuid,
}

#[derive(Debug, OneofObject)]
pub enum CatalogReleaseCommandInput {
    MarkMissing(CatalogCommandSignal),
    MarkWanted(CatalogCommandSignal),
    MarkUnavailable(CatalogCommandSignal),
    RecordCopy(RecordCollectionCopyInput),
    BeginIngest(BeginCatalogIngestInput),
    Publish(PublishCatalogReleaseInput),
    ReturnToAcquired(CatalogCommandSignal),
}

#[derive(Debug, InputObject)]
pub struct ExecuteCatalogReleaseCommandInput {
    release_id: Uuid,
    expected_row_version: String,
    command: CatalogReleaseCommandInput,
}

pub async fn query_artists(
    ctx: &Context<'_>,
    search: Option<String>,
    limit: i32,
    offset: i32,
) -> Result<Vec<CatalogArtistInfo>> {
    validate_pagination(limit, offset)?;
    ctx.data::<CatalogService>()?
        .list_artists(
            search.as_deref(),
            u64::try_from(limit).expect("validated positive i32"),
            u64::try_from(offset).expect("validated non-negative i32"),
        )
        .await
        .map(|values| values.into_iter().map(Into::into).collect())
        .map_err(catalog_error)
}

pub async fn query_artist_collection(
    ctx: &Context<'_>,
    artist_id: Uuid,
    state: Option<CatalogCollectionState>,
    limit: i32,
    offset: i32,
) -> Result<Option<CatalogArtistCollectionInfo>> {
    validate_pagination(limit, offset)?;
    ctx.data::<CatalogService>()?
        .artist_collection(artist_id, state.map(Into::into))
        .await
        .map(|value| {
            value.map(|value| {
                CatalogArtistCollectionInfo::from_snapshot(
                    value,
                    usize::try_from(limit).expect("validated positive i32"),
                    usize::try_from(offset).expect("validated non-negative i32"),
                )
            })
        })
        .map_err(catalog_error)
}

pub async fn create_artist(
    ctx: &Context<'_>,
    input: CreateCatalogArtistInput,
) -> Result<CatalogArtistInfo> {
    ctx.data::<CatalogService>()?
        .create_artist(NewCatalogArtist {
            artist_id: input.artist_id,
            display_name: input.display_name,
            sort_name: input.sort_name,
            notes: input.notes,
        })
        .await
        .map(Into::into)
        .map_err(catalog_error)
}

pub async fn update_artist(
    ctx: &Context<'_>,
    input: UpdateCatalogArtistInput,
) -> Result<CatalogArtistInfo> {
    let expected = parse_row_version(&input.expected_row_version)?;
    ctx.data::<CatalogService>()?
        .update_artist(
            input.artist_id,
            expected,
            UpdateCatalogArtist {
                display_name: input.display_name,
                sort_name: input.sort_name,
                notes: input.notes,
            },
        )
        .await
        .map(Into::into)
        .map_err(catalog_error)
}

pub async fn create_release(
    ctx: &Context<'_>,
    input: CreateCatalogReleaseInput,
) -> Result<CatalogReleaseInfo> {
    ctx.data::<CatalogService>()?
        .create_release(NewCatalogRelease {
            release_id: input.release_id,
            artist_id: input.artist_id,
            title: input.title,
            edition: input.edition,
            catalog: input.catalog,
            release_date: input.release_date,
            kind: input.kind.into(),
            notes: input.notes,
        })
        .await
        .map(Into::into)
        .map_err(catalog_error)
}

pub async fn update_release(
    ctx: &Context<'_>,
    input: UpdateCatalogReleaseInput,
) -> Result<CatalogReleaseInfo> {
    let expected = parse_row_version(&input.expected_row_version)?;
    ctx.data::<CatalogService>()?
        .update_release(
            input.release_id,
            expected,
            UpdateCatalogRelease {
                title: input.title,
                edition: input.edition,
                catalog: input.catalog,
                release_date: input.release_date,
                kind: input.kind.into(),
                notes: input.notes,
            },
        )
        .await
        .map(Into::into)
        .map_err(catalog_error)
}

pub async fn execute_release_command(
    ctx: &Context<'_>,
    input: ExecuteCatalogReleaseCommandInput,
) -> Result<CatalogReleaseInfo> {
    let expected = parse_row_version(&input.expected_row_version)?;
    let command = command_from_input(input.command)?;
    ctx.data::<CatalogService>()?
        .execute_release_command(input.release_id, expected, command)
        .await
        .map(Into::into)
        .map_err(catalog_error)
}

fn command_from_input(input: CatalogReleaseCommandInput) -> Result<CatalogReleaseCommand> {
    match input {
        CatalogReleaseCommandInput::MarkMissing(_) => Ok(CatalogReleaseCommand::MarkMissing),
        CatalogReleaseCommandInput::MarkWanted(_) => Ok(CatalogReleaseCommand::MarkWanted),
        CatalogReleaseCommandInput::MarkUnavailable(_) => {
            Ok(CatalogReleaseCommand::MarkUnavailable)
        }
        CatalogReleaseCommandInput::RecordCopy(input) => {
            Ok(CatalogReleaseCommand::RecordCopy(NewCollectionCopy {
                copy_id: input.copy_id,
                source_kind: input.source_kind.into(),
                source_label: input.source_label,
                private_locator: input.private_locator,
                codec: input.codec.into(),
                sample_rate_hz: parse_nonzero_u32("sampleRateHz", input.sample_rate_hz)?,
                bit_depth: parse_u8("bitDepth", input.bit_depth)?,
                channels: parse_u8("channels", input.channels)?,
                track_count: parse_u32("trackCount", input.track_count)?,
                byte_length: input
                    .byte_length
                    .as_deref()
                    .map(|value| parse_u64("byteLength", value))
                    .transpose()?,
                manifest_digest: input
                    .manifest_digest
                    .as_deref()
                    .map(parse_digest)
                    .transpose()?,
                quality_verified: input.quality_verified,
                ingest_job_id: input.ingest_job_id,
                notes: input.notes,
            }))
        }
        CatalogReleaseCommandInput::BeginIngest(input) => Ok(CatalogReleaseCommand::BeginIngest {
            job_id: input.job_id,
        }),
        CatalogReleaseCommandInput::Publish(input) => Ok(CatalogReleaseCommand::Publish {
            album_id: input.album_id,
        }),
        CatalogReleaseCommandInput::ReturnToAcquired(_) => {
            Ok(CatalogReleaseCommand::ReturnToAcquired)
        }
    }
}

fn validate_pagination(limit: i32, offset: i32) -> Result<()> {
    if !(1..=500).contains(&limit) || offset < 0 {
        return Err(input_error(
            "CATALOG_INVALID_PAGINATION",
            "limit must be between 1 and 500 and offset must be non-negative",
        ));
    }
    Ok(())
}

fn parse_row_version(value: &str) -> Result<CatalogRowVersion> {
    value
        .parse::<u64>()
        .ok()
        .and_then(CatalogRowVersion::new)
        .ok_or_else(|| {
            input_error(
                "CATALOG_INVALID_ROW_VERSION",
                "row version must be a positive base-10 integer",
            )
        })
}

fn parse_nonzero_u32(field: &'static str, value: Option<i32>) -> Result<Option<NonZeroU32>> {
    value
        .map(|value| {
            u32::try_from(value)
                .ok()
                .and_then(NonZeroU32::new)
                .ok_or_else(|| numeric_input_error(field))
        })
        .transpose()
}

fn parse_u8(field: &'static str, value: Option<i32>) -> Result<Option<u8>> {
    value
        .map(|value| {
            u8::try_from(value)
                .ok()
                .filter(|value| *value > 0)
                .ok_or_else(|| numeric_input_error(field))
        })
        .transpose()
}

fn parse_u32(field: &'static str, value: Option<i32>) -> Result<Option<u32>> {
    value
        .map(|value| {
            u32::try_from(value)
                .ok()
                .filter(|value| *value > 0)
                .ok_or_else(|| numeric_input_error(field))
        })
        .transpose()
}

fn parse_u64(field: &'static str, value: &str) -> Result<u64> {
    value
        .parse::<u64>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| numeric_input_error(field))
}

fn parse_digest(value: &str) -> Result<Digest> {
    value.parse().map_err(|error| {
        input_error(
            "CATALOG_INVALID_DIGEST",
            format!("invalid manifest digest: {error}"),
        )
    })
}

fn numeric_input_error(field: &'static str) -> Error {
    input_error(
        "CATALOG_INVALID_NUMBER",
        format!("{field} must be a positive integer in range"),
    )
}

fn input_error(code: &'static str, message: impl Into<String>) -> Error {
    Error::new(message).extend_with(|_, extensions| extensions.set("code", code))
}

fn catalog_error(error: CatalogError) -> Error {
    match error {
        CatalogError::ArtistAlreadyExists { artist_id } => {
            entity_error("CATALOG_ARTIST_ALREADY_EXISTS", "artistId", artist_id)
        }
        CatalogError::ArtistNotFound { artist_id } => {
            entity_error("CATALOG_ARTIST_NOT_FOUND", "artistId", artist_id)
        }
        CatalogError::ArtistConflict {
            artist_id,
            expected,
            actual,
        } => conflict_error(
            "CATALOG_ARTIST_CONFLICT",
            "artistId",
            artist_id,
            expected,
            actual,
        ),
        CatalogError::ReleaseAlreadyExists { release_id } => {
            entity_error("CATALOG_RELEASE_ALREADY_EXISTS", "releaseId", release_id)
        }
        CatalogError::ReleaseNotFound { release_id } => {
            entity_error("CATALOG_RELEASE_NOT_FOUND", "releaseId", release_id)
        }
        CatalogError::ReleaseConflict {
            release_id,
            expected,
            actual,
        } => conflict_error(
            "CATALOG_RELEASE_CONFLICT",
            "releaseId",
            release_id,
            expected,
            actual,
        ),
        CatalogError::CopyAlreadyExists { copy_id } => {
            entity_error("CATALOG_COPY_ALREADY_EXISTS", "copyId", copy_id)
        }
        CatalogError::InvalidTransition {
            release_id,
            from,
            to,
        } => Error::new(format!(
            "catalog release {release_id} cannot transition from {from} to {to}"
        ))
        .extend_with(|_, extensions| {
            extensions.set("code", "CATALOG_INVALID_TRANSITION");
            extensions.set("releaseId", release_id.to_string());
            extensions.set("from", from.as_str());
            extensions.set("to", to.as_str());
        }),
        CatalogError::NoAcquiredCopy { release_id } => {
            entity_error("CATALOG_NO_ACQUIRED_COPY", "releaseId", release_id)
        }
        CatalogError::InvalidInput { field, message } => {
            Error::new(message).extend_with(|_, extensions| {
                extensions.set("code", "CATALOG_INVALID_INPUT");
                extensions.set("field", field);
            })
        }
        CatalogError::NumericOutOfRange { field, .. } => {
            Error::new("numeric value is out of range").extend_with(|_, extensions| {
                extensions.set("code", "CATALOG_INVALID_NUMBER");
                extensions.set("field", field);
            })
        }
        error @ CatalogError::InvalidPersistedValue { .. } => {
            tracing::error!(error = ?error, "persisted catalog data is corrupt");
            Error::new("persisted catalog data is corrupt")
                .extend_with(|_, extensions| extensions.set("code", "CATALOG_CORRUPT"))
        }
        CatalogError::Database(error) => {
            tracing::error!(error = ?error, "catalog database operation failed");
            Error::new("catalog operation failed")
                .extend_with(|_, extensions| extensions.set("code", "CATALOG_INTERNAL"))
        }
    }
}

fn entity_error(code: &'static str, id_field: &'static str, id: Uuid) -> Error {
    Error::new(code).extend_with(|_, extensions| {
        extensions.set("code", code);
        extensions.set(id_field, id.to_string());
    })
}

fn conflict_error(
    code: &'static str,
    id_field: &'static str,
    id: Uuid,
    expected: CatalogRowVersion,
    actual: CatalogRowVersion,
) -> Error {
    Error::new(code).extend_with(|_, extensions| {
        extensions.set("code", code);
        extensions.set(id_field, id.to_string());
        extensions.set("expectedRowVersion", expected.to_string());
        extensions.set("actualRowVersion", actual.to_string());
    })
}
