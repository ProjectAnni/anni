use anni_catalog::{CoverCandidateState, CoverMediaType, CoverSourceKind};
use async_graphql::{Context, Enum, Error, ErrorExtensions, InputObject, Result, SimpleObject};
use chrono::{DateTime, Utc};
use sea_orm::prelude::Uuid;

use crate::cover::{
    CoverAssetSnapshot, CoverCandidateSnapshot, CoverError, CoverRowVersion,
    CoverSelectionSnapshot, CoverService, NewCoverCandidate, SelectCover,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum CoverSource {
    AppleMusic,
    Amazon,
    RecordLabel,
    ArtistWebsite,
    Vgmdb,
    Manual,
}

impl From<CoverSourceKind> for CoverSource {
    fn from(value: CoverSourceKind) -> Self {
        match value {
            CoverSourceKind::AppleMusic => Self::AppleMusic,
            CoverSourceKind::Amazon => Self::Amazon,
            CoverSourceKind::RecordLabel => Self::RecordLabel,
            CoverSourceKind::ArtistWebsite => Self::ArtistWebsite,
            CoverSourceKind::Vgmdb => Self::Vgmdb,
            CoverSourceKind::Manual => Self::Manual,
        }
    }
}

impl From<CoverSource> for CoverSourceKind {
    fn from(value: CoverSource) -> Self {
        match value {
            CoverSource::AppleMusic => Self::AppleMusic,
            CoverSource::Amazon => Self::Amazon,
            CoverSource::RecordLabel => Self::RecordLabel,
            CoverSource::ArtistWebsite => Self::ArtistWebsite,
            CoverSource::Vgmdb => Self::Vgmdb,
            CoverSource::Manual => Self::Manual,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum CoverCandidateStatus {
    Discovered,
    Queued,
    Fetching,
    Verified,
    Rejected,
}

impl From<CoverCandidateState> for CoverCandidateStatus {
    fn from(value: CoverCandidateState) -> Self {
        match value {
            CoverCandidateState::Discovered => Self::Discovered,
            CoverCandidateState::Queued => Self::Queued,
            CoverCandidateState::Fetching => Self::Fetching,
            CoverCandidateState::Verified => Self::Verified,
            CoverCandidateState::Rejected => Self::Rejected,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum CoverAssetMediaType {
    Jpeg,
    Png,
    Webp,
}

impl From<CoverMediaType> for CoverAssetMediaType {
    fn from(value: CoverMediaType) -> Self {
        match value {
            CoverMediaType::Jpeg => Self::Jpeg,
            CoverMediaType::Png => Self::Png,
            CoverMediaType::Webp => Self::Webp,
        }
    }
}

/// Verified image metadata safe to return to the browser.
///
/// Storage locators are deliberately omitted. Integer values which do not fit
/// GraphQL's signed 32-bit `Int` are represented as base-10 strings.
#[derive(Debug, Clone, PartialEq, Eq, SimpleObject)]
pub struct CoverAssetInfo {
    asset_id: Uuid,
    content_sha256: String,
    media_type: CoverAssetMediaType,
    width: String,
    height: String,
    byte_length: String,
    fetched_at: DateTime<Utc>,
    verified_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
}

impl From<CoverAssetSnapshot> for CoverAssetInfo {
    fn from(value: CoverAssetSnapshot) -> Self {
        Self {
            asset_id: value.asset_id,
            content_sha256: value.content_sha256.to_string(),
            media_type: value.media_type.into(),
            width: value.width.to_string(),
            height: value.height.to_string(),
            byte_length: value.byte_length.to_string(),
            fetched_at: value.fetched_at,
            verified_at: value.verified_at,
            created_at: value.created_at,
        }
    }
}

/// Browser-safe candidate information. It contains URL presence only, never
/// the submitted, canonical, effective, or worker request URL.
#[derive(Debug, Clone, PartialEq, Eq, SimpleObject)]
pub struct CoverCandidateInfo {
    candidate_id: Uuid,
    release_id: Uuid,
    disc_number: i32,
    source_kind: CoverSource,
    source_release_revision_db_id: Option<i32>,
    state: CoverCandidateStatus,
    asset: Option<CoverAssetInfo>,
    has_remote_url: bool,
    attempt_count: String,
    next_attempt_at: Option<DateTime<Utc>>,
    last_http_status: Option<i32>,
    last_error_code: Option<String>,
    fetched_at: Option<DateTime<Utc>>,
    row_version: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<CoverCandidateSnapshot> for CoverCandidateInfo {
    fn from(value: CoverCandidateSnapshot) -> Self {
        Self {
            candidate_id: value.candidate_id,
            release_id: value.release_id,
            disc_number: i32::from(value.disc_number),
            source_kind: value.source_kind.into(),
            source_release_revision_db_id: value.source_release_revision_db_id,
            state: value.state.into(),
            asset: value.asset.map(Into::into),
            has_remote_url: value.has_remote_url,
            attempt_count: value.attempt_count.to_string(),
            next_attempt_at: value.next_attempt_at,
            last_http_status: value.last_http_status.map(i32::from),
            last_error_code: value.last_error_code,
            fetched_at: value.fetched_at,
            row_version: value.row_version.to_string(),
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, SimpleObject)]
pub struct CoverSelectionInfo {
    selection_id: Uuid,
    release_id: Uuid,
    disc_number: i32,
    candidate_id: Uuid,
    asset: CoverAssetInfo,
    row_version: String,
    selected_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<CoverSelectionSnapshot> for CoverSelectionInfo {
    fn from(value: CoverSelectionSnapshot) -> Self {
        Self {
            selection_id: value.selection_id,
            release_id: value.release_id,
            disc_number: i32::from(value.disc_number),
            candidate_id: value.candidate_id,
            asset: value.asset.into(),
            row_version: value.row_version.to_string(),
            selected_at: value.selected_at,
            updated_at: value.updated_at,
        }
    }
}

#[derive(Clone, PartialEq, Eq, InputObject)]
pub struct AddCoverCandidateInput {
    candidate_id: Option<Uuid>,
    release_id: Uuid,
    /// Zero means release-level artwork; positive values identify discs.
    disc_number: i32,
    source_kind: CoverSource,
    source_release_revision_db_id: Option<i32>,
    submitted_url: String,
}

impl std::fmt::Debug for AddCoverCandidateInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AddCoverCandidateInput")
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

#[derive(Debug, InputObject)]
pub struct QueueCoverCandidateInput {
    candidate_id: Uuid,
    expected_row_version: String,
    not_before: Option<DateTime<Utc>>,
}

#[derive(InputObject)]
pub struct RejectCoverCandidateInput {
    candidate_id: Uuid,
    expected_row_version: String,
    reason_code: String,
}

impl std::fmt::Debug for RejectCoverCandidateInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RejectCoverCandidateInput")
            .field("candidate_id", &self.candidate_id)
            .field("expected_row_version", &self.expected_row_version)
            .field("reason_code", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, InputObject)]
pub struct SelectCoverInput {
    release_id: Uuid,
    disc_number: i32,
    candidate_id: Uuid,
    /// Omit when creating the first selection. Supply the current version when
    /// replacing an existing selection.
    expected_row_version: Option<String>,
}

pub async fn query_candidates(
    ctx: &Context<'_>,
    release_id: Uuid,
    disc_number: Option<i32>,
) -> Result<Vec<CoverCandidateInfo>> {
    let disc_number = disc_number.map(parse_disc_number).transpose()?;
    ctx.data::<CoverService>()?
        .list_candidates(release_id, disc_number)
        .await
        .map(|values| values.into_iter().map(Into::into).collect())
        .map_err(cover_error)
}

pub async fn query_selection(
    ctx: &Context<'_>,
    release_id: Uuid,
    disc_number: i32,
) -> Result<Option<CoverSelectionInfo>> {
    ctx.data::<CoverService>()?
        .get_selection(release_id, parse_disc_number(disc_number)?)
        .await
        .map(|value| value.map(Into::into))
        .map_err(cover_error)
}

pub async fn add_candidate(
    ctx: &Context<'_>,
    input: AddCoverCandidateInput,
) -> Result<CoverCandidateInfo> {
    let disc_number = parse_disc_number(input.disc_number)?;
    let source_release_revision_db_id = input
        .source_release_revision_db_id
        .map(parse_source_revision_id)
        .transpose()?;
    ctx.data::<CoverService>()?
        .create_candidate(NewCoverCandidate {
            candidate_id: input.candidate_id,
            release_id: input.release_id,
            disc_number,
            source_kind: input.source_kind.into(),
            source_release_revision_db_id,
            submitted_url: input.submitted_url,
        })
        .await
        .map(Into::into)
        .map_err(cover_error)
}

pub async fn queue_candidate(
    ctx: &Context<'_>,
    input: QueueCoverCandidateInput,
) -> Result<CoverCandidateInfo> {
    let expected = parse_row_version(&input.expected_row_version)?;
    ctx.data::<CoverService>()?
        .queue_candidate(input.candidate_id, expected, input.not_before)
        .await
        .map(Into::into)
        .map_err(cover_error)
}

pub async fn reject_candidate(
    ctx: &Context<'_>,
    input: RejectCoverCandidateInput,
) -> Result<CoverCandidateInfo> {
    let expected = parse_row_version(&input.expected_row_version)?;
    ctx.data::<CoverService>()?
        .reject_candidate(input.candidate_id, expected, input.reason_code)
        .await
        .map(Into::into)
        .map_err(cover_error)
}

pub async fn select_cover(
    ctx: &Context<'_>,
    input: SelectCoverInput,
) -> Result<CoverSelectionInfo> {
    let disc_number = parse_disc_number(input.disc_number)?;
    let expected_row_version = input
        .expected_row_version
        .as_deref()
        .map(parse_row_version)
        .transpose()?;
    ctx.data::<CoverService>()?
        .select_cover(SelectCover {
            release_id: input.release_id,
            disc_number,
            candidate_id: input.candidate_id,
            expected_row_version,
        })
        .await
        .map(Into::into)
        .map_err(cover_error)
}

fn parse_disc_number(value: i32) -> Result<u16> {
    u16::try_from(value)
        .ok()
        .filter(|value| *value <= i16::MAX as u16)
        .ok_or_else(|| {
            input_error(
                "COVER_INVALID_DISC_NUMBER",
                "disc number must be between 0 and 32767",
            )
        })
}

fn parse_source_revision_id(value: i32) -> Result<i32> {
    (value > 0).then_some(value).ok_or_else(|| {
        input_error(
            "COVER_INVALID_SOURCE_REVISION",
            "source release revision id must be positive",
        )
    })
}

fn parse_row_version(value: &str) -> Result<CoverRowVersion> {
    value
        .parse::<u64>()
        .ok()
        .and_then(CoverRowVersion::new)
        .ok_or_else(|| {
            input_error(
                "COVER_INVALID_ROW_VERSION",
                "row version must be a positive base-10 integer",
            )
        })
}

fn input_error(code: &'static str, message: impl Into<String>) -> Error {
    Error::new(message).extend_with(|_, extensions| extensions.set("code", code))
}

fn cover_error(error: CoverError) -> Error {
    match error {
        CoverError::ReleaseNotFound { release_id } => {
            entity_error("COVER_RELEASE_NOT_FOUND", "releaseId", release_id)
        }
        CoverError::CandidateAlreadyExists { candidate_id } => entity_error(
            "COVER_CANDIDATE_ALREADY_EXISTS",
            "candidateId",
            candidate_id,
        ),
        CoverError::CandidateNotFound { candidate_id } => {
            entity_error("COVER_CANDIDATE_NOT_FOUND", "candidateId", candidate_id)
        }
        CoverError::CandidateConflict {
            candidate_id,
            expected,
            actual,
        } => conflict_error(
            "COVER_CANDIDATE_CONFLICT",
            "candidateId",
            candidate_id,
            expected,
            actual,
        ),
        CoverError::InvalidCandidateTransition {
            candidate_id,
            from,
            to,
        } => Error::new(format!(
            "cover candidate {candidate_id} cannot transition from {from} to {to}"
        ))
        .extend_with(|_, extensions| {
            extensions.set("code", "COVER_INVALID_TRANSITION");
            extensions.set("candidateId", candidate_id.to_string());
            extensions.set("from", from.as_str());
            extensions.set("to", to.as_str());
        }),
        CoverError::CandidateNotVerified { candidate_id } => {
            entity_error("COVER_CANDIDATE_NOT_VERIFIED", "candidateId", candidate_id)
        }
        CoverError::CandidateScopeMismatch {
            candidate_id,
            expected_release_id,
            expected_disc_number,
            actual_release_id,
            actual_disc_number,
        } => Error::new("cover candidate belongs to a different release or disc").extend_with(
            |_, extensions| {
                extensions.set("code", "COVER_CANDIDATE_SCOPE_MISMATCH");
                extensions.set("candidateId", candidate_id.to_string());
                extensions.set("expectedReleaseId", expected_release_id.to_string());
                extensions.set("expectedDiscNumber", i32::from(expected_disc_number));
                extensions.set("actualReleaseId", actual_release_id.to_string());
                extensions.set("actualDiscNumber", i32::from(actual_disc_number));
            },
        ),
        CoverError::SelectionAlreadyExists {
            release_id,
            disc_number,
        } => selection_error("COVER_SELECTION_ALREADY_EXISTS", release_id, disc_number),
        CoverError::SelectionNotFound {
            release_id,
            disc_number,
        } => selection_error("COVER_SELECTION_NOT_FOUND", release_id, disc_number),
        CoverError::SelectionConflict {
            release_id,
            disc_number,
            expected,
            actual,
        } => Error::new("COVER_SELECTION_CONFLICT").extend_with(|_, extensions| {
            extensions.set("code", "COVER_SELECTION_CONFLICT");
            extensions.set("releaseId", release_id.to_string());
            extensions.set("discNumber", i32::from(disc_number));
            extensions.set("expectedRowVersion", expected.to_string());
            extensions.set("actualRowVersion", actual.to_string());
        }),
        CoverError::InvalidUrl(_) => input_error("COVER_INVALID_URL", "cover URL is invalid"),
        CoverError::InvalidInput { field, message } => {
            Error::new(message).extend_with(|_, extensions| {
                extensions.set("code", "COVER_INVALID_INPUT");
                extensions.set("field", field);
            })
        }
        CoverError::NumericOutOfRange { field, .. } => Error::new("numeric value is out of range")
            .extend_with(|_, extensions| {
                extensions.set("code", "COVER_INVALID_NUMBER");
                extensions.set("field", field);
            }),
        error @ (CoverError::LeaseMismatch { .. }
        | CoverError::CandidateAssetMissing { .. }
        | CoverError::InvalidPersistedValue { .. }
        | CoverError::AssetDigestConflict { .. }
        | CoverError::Database(_)) => {
            tracing::error!(error = ?error, "cover operation failed internally");
            Error::new("cover operation failed")
                .extend_with(|_, extensions| extensions.set("code", "COVER_INTERNAL"))
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
    expected: CoverRowVersion,
    actual: CoverRowVersion,
) -> Error {
    Error::new(code).extend_with(|_, extensions| {
        extensions.set("code", code);
        extensions.set(id_field, id.to_string());
        extensions.set("expectedRowVersion", expected.to_string());
        extensions.set("actualRowVersion", actual.to_string());
    })
}

fn selection_error(code: &'static str, release_id: Uuid, disc_number: u16) -> Error {
    Error::new(code).extend_with(|_, extensions| {
        extensions.set("code", code);
        extensions.set("releaseId", release_id.to_string());
        extensions.set("discNumber", i32::from(disc_number));
    })
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use anni_ingest::Digest;

    use super::*;

    #[test]
    fn browser_types_are_lossless_without_leaking_the_submitted_url() {
        let timestamp = DateTime::<Utc>::from_timestamp(1_700_000_000, 0).unwrap();
        let digest = Digest::new([0xab; 32]);
        let asset = CoverAssetSnapshot {
            asset_id: Uuid::new_v4(),
            content_sha256: digest,
            storage_key: "private/storage/key.jpg".to_owned(),
            media_type: CoverMediaType::Jpeg,
            width: NonZeroU32::new(u32::MAX).unwrap(),
            height: NonZeroU32::new(1).unwrap(),
            byte_length: u64::MAX,
            fetched_at: timestamp,
            verified_at: timestamp,
            created_at: timestamp,
        };
        let candidate = CoverCandidateInfo::from(CoverCandidateSnapshot {
            candidate_id: Uuid::new_v4(),
            release_id: Uuid::new_v4(),
            disc_number: 1,
            source_kind: CoverSourceKind::ArtistWebsite,
            source_release_revision_db_id: Some(7),
            state: CoverCandidateState::Verified,
            asset: Some(asset),
            has_remote_url: true,
            attempt_count: u32::MAX,
            next_attempt_at: None,
            last_http_status: Some(200),
            last_error_code: None,
            fetched_at: Some(timestamp),
            row_version: CoverRowVersion::new(u64::MAX).unwrap(),
            created_at: timestamp,
            updated_at: timestamp,
        });

        let asset = candidate.asset.unwrap();
        assert_eq!(asset.content_sha256, "ab".repeat(32));
        assert_eq!(asset.width, u32::MAX.to_string());
        assert_eq!(asset.byte_length, u64::MAX.to_string());
        assert_eq!(candidate.attempt_count, u32::MAX.to_string());
        assert_eq!(candidate.row_version, u64::MAX.to_string());
        assert!(!format!("{asset:?}").contains("private/storage"));

        let submitted_url = "https://example.invalid/cover.jpg?token=secret".to_owned();
        let input = AddCoverCandidateInput {
            candidate_id: None,
            release_id: Uuid::new_v4(),
            disc_number: 0,
            source_kind: CoverSource::Manual,
            source_release_revision_db_id: None,
            submitted_url: submitted_url.clone(),
        };
        let debug = format!("{input:?}");
        assert!(!debug.contains(&submitted_url));
        assert!(!debug.contains("token=secret"));
        assert!(debug.contains("[REDACTED]"));
    }

    #[test]
    fn graphql_number_parsers_reject_lossy_or_invalid_values() {
        assert_eq!(parse_disc_number(0).unwrap(), 0);
        assert_eq!(parse_disc_number(i16::MAX.into()).unwrap(), i16::MAX as u16);
        assert!(parse_disc_number(-1).is_err());
        assert!(parse_disc_number(i32::from(i16::MAX) + 1).is_err());
        assert!(parse_source_revision_id(0).is_err());
        assert!(parse_row_version("18446744073709551615").is_ok());
        assert!(parse_row_version("0").is_err());
        assert!(parse_row_version("01x").is_err());
    }
}
