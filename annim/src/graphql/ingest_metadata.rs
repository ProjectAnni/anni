//! Strongly typed GraphQL boundary for metadata evidence and review.
//!
//! Inputs are converted into `anni-ingest` domain values before persistence.
//! Metadata text is copied exactly; validation never trims or normalizes it.

use std::{collections::BTreeMap, num::NonZeroU16, str::FromStr};

use anni_ingest::{
    AlbumField, AlbumLayout, Confidence, DiscField, Evidence, EvidenceMethod, EvidenceSourceKind,
    FieldPath, MetadataCandidate, MetadataDecision, MetadataDraft, MetadataError, MetadataProfile,
    MetadataReviewContext, MetadataValue, TrackField,
};
use anni_metadata::model::{AnniDate, TrackType};
use async_graphql::{
    Context, Enum, Error, ErrorExtensions, InputObject, OneofObject, Result, SimpleObject,
};
use chrono::{DateTime, Utc};
use sea_orm::prelude::Uuid;

use crate::ingest::{
    IngestCommand, IngestMetadataReview, IngestService, MetadataEdit, PersistedMetadataDraft,
};

use super::ingest::{input_error, parse_revision, parse_row_version, service_error, IngestJobInfo};

const MAX_DISCS_PER_REVIEW: usize = 100;
const MAX_TOTAL_TRACKS_PER_REVIEW: u32 = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum IngestMetadataProfile {
    Cd,
    Streaming,
}

impl From<IngestMetadataProfile> for MetadataProfile {
    fn from(value: IngestMetadataProfile) -> Self {
        match value {
            IngestMetadataProfile::Cd => Self::Cd,
            IngestMetadataProfile::Streaming => Self::Streaming,
        }
    }
}

impl From<MetadataProfile> for IngestMetadataProfile {
    fn from(value: MetadataProfile) -> Self {
        match value {
            MetadataProfile::Cd => Self::Cd,
            MetadataProfile::Streaming => Self::Streaming,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum IngestMetadataFieldScope {
    Album,
    Disc,
    Track,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum IngestMetadataFieldName {
    Title,
    Edition,
    Artist,
    Artists,
    ReleaseDate,
    TrackType,
    Catalog,
    Tags,
}

#[derive(Debug, Clone, PartialEq, Eq, InputObject)]
pub struct IngestMetadataFieldInput {
    scope: IngestMetadataFieldScope,
    disc: Option<i32>,
    track: Option<i32>,
    field: IngestMetadataFieldName,
}

impl TryFrom<IngestMetadataFieldInput> for FieldPath {
    type Error = Error;

    fn try_from(input: IngestMetadataFieldInput) -> Result<Self> {
        match input.scope {
            IngestMetadataFieldScope::Album => {
                if input.disc.is_some() || input.track.is_some() {
                    return Err(invalid_field("album fields cannot include disc or track"));
                }
                album_field(input.field)
                    .map(FieldPath::Album)
                    .ok_or_else(|| invalid_field("field is not valid at album scope"))
            }
            IngestMetadataFieldScope::Disc => {
                if input.track.is_some() {
                    return Err(invalid_field("disc fields cannot include a track"));
                }
                let disc = parse_position("disc", input.disc)?;
                let field = disc_field(input.field)
                    .ok_or_else(|| invalid_field("field is not valid at disc scope"))?;
                Ok(FieldPath::Disc { disc, field })
            }
            IngestMetadataFieldScope::Track => {
                let disc = parse_position("disc", input.disc)?;
                let track = parse_position("track", input.track)?;
                let field = track_field(input.field)
                    .ok_or_else(|| invalid_field("field is not valid at track scope"))?;
                Ok(FieldPath::Track { disc, track, field })
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, SimpleObject)]
pub struct IngestMetadataFieldInfo {
    scope: IngestMetadataFieldScope,
    disc: Option<i32>,
    track: Option<i32>,
    field: IngestMetadataFieldName,
}

impl From<FieldPath> for IngestMetadataFieldInfo {
    fn from(value: FieldPath) -> Self {
        match value {
            FieldPath::Album(field) => Self {
                scope: IngestMetadataFieldScope::Album,
                disc: None,
                track: None,
                field: album_field_name(field),
            },
            FieldPath::Disc { disc, field } => Self {
                scope: IngestMetadataFieldScope::Disc,
                disc: Some(i32::from(disc.get())),
                track: None,
                field: disc_field_name(field),
            },
            FieldPath::Track { disc, track, field } => Self {
                scope: IngestMetadataFieldScope::Track,
                disc: Some(i32::from(disc.get())),
                track: Some(i32::from(track.get())),
                field: track_field_name(field),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum IngestMetadataTrackType {
    Normal,
    Instrumental,
    Absolute,
    Drama,
    Radio,
    Vocal,
}

impl From<IngestMetadataTrackType> for TrackType {
    fn from(value: IngestMetadataTrackType) -> Self {
        match value {
            IngestMetadataTrackType::Normal => Self::Normal,
            IngestMetadataTrackType::Instrumental => Self::Instrumental,
            IngestMetadataTrackType::Absolute => Self::Absolute,
            IngestMetadataTrackType::Drama => Self::Drama,
            IngestMetadataTrackType::Radio => Self::Radio,
            IngestMetadataTrackType::Vocal => Self::Vocal,
        }
    }
}

impl From<&TrackType> for IngestMetadataTrackType {
    fn from(value: &TrackType) -> Self {
        match value {
            TrackType::Normal => Self::Normal,
            TrackType::Instrumental => Self::Instrumental,
            TrackType::Absolute => Self::Absolute,
            TrackType::Drama => Self::Drama,
            TrackType::Radio => Self::Radio,
            TrackType::Vocal => Self::Vocal,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, InputObject)]
pub struct IngestMetadataTextListInput {
    values: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, InputObject)]
pub struct IngestMetadataTextMapEntryInput {
    key: String,
    value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, InputObject)]
pub struct IngestMetadataTextMapInput {
    entries: Vec<IngestMetadataTextMapEntryInput>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum IngestMetadataAbsentSignal {
    Use,
}

#[derive(Debug, Clone, PartialEq, Eq, OneofObject)]
pub enum IngestMetadataValueInput {
    Text(String),
    Date(String),
    TrackType(IngestMetadataTrackType),
    TextList(IngestMetadataTextListInput),
    TextMap(IngestMetadataTextMapInput),
    Absent(IngestMetadataAbsentSignal),
}

impl TryFrom<IngestMetadataValueInput> for MetadataValue {
    type Error = Error;

    fn try_from(input: IngestMetadataValueInput) -> Result<Self> {
        match input {
            IngestMetadataValueInput::Text(value) => Ok(Self::Text(value)),
            IngestMetadataValueInput::Date(value) => {
                let valid_shape = matches!(value.len(), 4 | 7 | 10)
                    && value.split('-').enumerate().all(|(index, part)| {
                        part.len() == if index == 0 { 4 } else { 2 }
                            && part.bytes().all(|byte| byte.is_ascii_digit())
                    })
                    && value.as_bytes().get(4).is_none_or(|byte| *byte == b'-')
                    && value.as_bytes().get(7).is_none_or(|byte| *byte == b'-');
                if !valid_shape {
                    return Err(input_error(
                        "INGEST_METADATA_INVALID_VALUE",
                        "date must use YYYY, YYYY-MM, or YYYY-MM-DD",
                    ));
                }
                let date = AnniDate::from_str(&value).map_err(|_| {
                    input_error(
                        "INGEST_METADATA_INVALID_VALUE",
                        "date must use YYYY, YYYY-MM, or YYYY-MM-DD",
                    )
                })?;
                validate_date(&date)?;
                Ok(Self::Date(date))
            }
            IngestMetadataValueInput::TrackType(value) => Ok(Self::TrackType(value.into())),
            IngestMetadataValueInput::TextList(input) => Ok(Self::TextList(input.values)),
            IngestMetadataValueInput::TextMap(input) => {
                let mut values = BTreeMap::new();
                for entry in input.entries {
                    if values.insert(entry.key.clone(), entry.value).is_some() {
                        return Err(input_error(
                            "INGEST_METADATA_INVALID_VALUE",
                            format!("text map contains duplicate key {:?}", entry.key),
                        ));
                    }
                }
                Ok(Self::TextMap(values))
            }
            IngestMetadataValueInput::Absent(_) => Ok(Self::Absent),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum IngestMetadataValueKind {
    Text,
    Date,
    TrackType,
    TextList,
    TextMap,
    Absent,
}

#[derive(Debug, Clone, PartialEq, Eq, SimpleObject)]
pub struct IngestMetadataTextMapEntry {
    key: String,
    value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, SimpleObject)]
pub struct IngestMetadataValueInfo {
    kind: IngestMetadataValueKind,
    text: Option<String>,
    date: Option<String>,
    track_type: Option<IngestMetadataTrackType>,
    text_list: Vec<String>,
    text_map: Vec<IngestMetadataTextMapEntry>,
}

impl From<&MetadataValue> for IngestMetadataValueInfo {
    fn from(value: &MetadataValue) -> Self {
        let mut result = Self {
            kind: IngestMetadataValueKind::Absent,
            text: None,
            date: None,
            track_type: None,
            text_list: Vec::new(),
            text_map: Vec::new(),
        };
        match value {
            MetadataValue::Text(value) => {
                result.kind = IngestMetadataValueKind::Text;
                result.text = Some(value.clone());
            }
            MetadataValue::Date(value) => {
                result.kind = IngestMetadataValueKind::Date;
                result.date = Some(value.to_string());
            }
            MetadataValue::TrackType(value) => {
                result.kind = IngestMetadataValueKind::TrackType;
                result.track_type = Some(value.into());
            }
            MetadataValue::TextList(values) => {
                result.kind = IngestMetadataValueKind::TextList;
                result.text_list.clone_from(values);
            }
            MetadataValue::TextMap(values) => {
                result.kind = IngestMetadataValueKind::TextMap;
                result.text_map = values
                    .iter()
                    .map(|(key, value)| IngestMetadataTextMapEntry {
                        key: key.clone(),
                        value: value.clone(),
                    })
                    .collect();
            }
            MetadataValue::Absent => {}
        }
        result
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum IngestEvidenceSourceKind {
    CdBooklet,
    CdPackaging,
    OfficialLabel,
    OfficialArtist,
    OfficialStore,
    StreamingService,
    Vgmdb,
    CommunitySource,
    Filename,
    DerivedInference,
}

impl From<IngestEvidenceSourceKind> for EvidenceSourceKind {
    fn from(value: IngestEvidenceSourceKind) -> Self {
        match value {
            IngestEvidenceSourceKind::CdBooklet => Self::CdBooklet,
            IngestEvidenceSourceKind::CdPackaging => Self::CdPackaging,
            IngestEvidenceSourceKind::OfficialLabel => Self::OfficialLabel,
            IngestEvidenceSourceKind::OfficialArtist => Self::OfficialArtist,
            IngestEvidenceSourceKind::OfficialStore => Self::OfficialStore,
            IngestEvidenceSourceKind::StreamingService => Self::StreamingService,
            IngestEvidenceSourceKind::Vgmdb => Self::Vgmdb,
            IngestEvidenceSourceKind::CommunitySource => Self::CommunitySource,
            IngestEvidenceSourceKind::Filename => Self::Filename,
            IngestEvidenceSourceKind::DerivedInference => Self::DerivedInference,
        }
    }
}

impl From<EvidenceSourceKind> for IngestEvidenceSourceKind {
    fn from(value: EvidenceSourceKind) -> Self {
        match value {
            EvidenceSourceKind::CdBooklet => Self::CdBooklet,
            EvidenceSourceKind::CdPackaging => Self::CdPackaging,
            EvidenceSourceKind::OfficialLabel => Self::OfficialLabel,
            EvidenceSourceKind::OfficialArtist => Self::OfficialArtist,
            EvidenceSourceKind::OfficialStore => Self::OfficialStore,
            EvidenceSourceKind::StreamingService => Self::StreamingService,
            EvidenceSourceKind::Vgmdb => Self::Vgmdb,
            EvidenceSourceKind::CommunitySource => Self::CommunitySource,
            EvidenceSourceKind::Filename => Self::Filename,
            EvidenceSourceKind::DerivedInference => Self::DerivedInference,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum IngestEvidenceMethod {
    ManualTranscription,
    AutomatedExtraction,
    WebImport,
    Inference,
}

impl From<IngestEvidenceMethod> for EvidenceMethod {
    fn from(value: IngestEvidenceMethod) -> Self {
        match value {
            IngestEvidenceMethod::ManualTranscription => Self::ManualTranscription,
            IngestEvidenceMethod::AutomatedExtraction => Self::AutomatedExtraction,
            IngestEvidenceMethod::WebImport => Self::WebImport,
            IngestEvidenceMethod::Inference => Self::Inference,
        }
    }
}

impl From<EvidenceMethod> for IngestEvidenceMethod {
    fn from(value: EvidenceMethod) -> Self {
        match value {
            EvidenceMethod::ManualTranscription => Self::ManualTranscription,
            EvidenceMethod::AutomatedExtraction => Self::AutomatedExtraction,
            EvidenceMethod::WebImport => Self::WebImport,
            EvidenceMethod::Inference => Self::Inference,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, InputObject)]
pub struct IngestEvidenceInput {
    source_kind: IngestEvidenceSourceKind,
    locator: String,
    detail: Option<String>,
    method: IngestEvidenceMethod,
}

impl TryFrom<IngestEvidenceInput> for Evidence {
    type Error = Error;

    fn try_from(input: IngestEvidenceInput) -> Result<Self> {
        if input.locator.trim().is_empty() {
            return Err(input_error(
                "INGEST_METADATA_INVALID_EVIDENCE",
                "evidence locator cannot be empty",
            ));
        }
        Ok(Self::new(
            input.source_kind.into(),
            input.locator,
            input.detail,
            input.method.into(),
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, SimpleObject)]
pub struct IngestEvidenceInfo {
    source_kind: IngestEvidenceSourceKind,
    locator: String,
    detail: Option<String>,
    method: IngestEvidenceMethod,
}

impl From<&Evidence> for IngestEvidenceInfo {
    fn from(value: &Evidence) -> Self {
        Self {
            source_kind: value.source_kind().into(),
            locator: value.locator().to_owned(),
            detail: value.detail().map(str::to_owned),
            method: value.method().into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum IngestMetadataDecision {
    Pending,
    Accepted,
    Rejected,
}

impl From<MetadataDecision> for IngestMetadataDecision {
    fn from(value: MetadataDecision) -> Self {
        match value {
            MetadataDecision::Pending => Self::Pending,
            MetadataDecision::Accepted => Self::Accepted,
            MetadataDecision::Rejected => Self::Rejected,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, SimpleObject)]
pub struct IngestMetadataCandidateInfo {
    candidate_id: Uuid,
    field: IngestMetadataFieldInfo,
    value: IngestMetadataValueInfo,
    evidence: IngestEvidenceInfo,
    confidence_basis_points: i32,
    decision: IngestMetadataDecision,
    recommended: bool,
}

impl IngestMetadataCandidateInfo {
    fn from_candidate(candidate: &MetadataCandidate, draft: &MetadataDraft) -> Self {
        Self {
            candidate_id: candidate.id(),
            field: candidate.field().into(),
            value: candidate.value().into(),
            evidence: candidate.evidence().into(),
            confidence_basis_points: i32::from(candidate.confidence().basis_points()),
            decision: draft
                .decision(candidate.id())
                .expect("candidate belongs to draft")
                .into(),
            recommended: draft
                .recommendation(candidate.field())
                .is_some_and(|recommended| recommended.id() == candidate.id()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, SimpleObject)]
pub struct IngestMetadataDraftInfo {
    revision: String,
    profile: Option<IngestMetadataProfile>,
    track_counts: Vec<i32>,
    candidates: Vec<IngestMetadataCandidateInfo>,
    requirements_configured: bool,
    total_required: Option<String>,
    accepted_required: Option<String>,
    missing_fields: Vec<IngestMetadataFieldInfo>,
    complete: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<&PersistedMetadataDraft> for IngestMetadataDraftInfo {
    fn from(value: &PersistedMetadataDraft) -> Self {
        let draft = value.draft();
        let (profile, track_counts) = draft
            .review_context()
            .map(|context| {
                (
                    Some(context.profile().into()),
                    context
                        .layout()
                        .track_counts()
                        .iter()
                        .map(|count| i32::from(count.get()))
                        .collect(),
                )
            })
            .unwrap_or_else(|| (None, Vec::new()));
        let completeness = draft.review_context().map(|context| {
            let requirements = context.requirements();
            draft.completeness(&requirements)
        });
        let (total_required, accepted_required, missing_fields, complete) = completeness
            .map(|report| {
                (
                    Some(report.total_required().to_string()),
                    Some(report.accepted_required().to_string()),
                    report.missing().iter().copied().map(Into::into).collect(),
                    report.is_complete(),
                )
            })
            .unwrap_or_else(|| (None, None, Vec::new(), false));

        Self {
            revision: draft.revision().to_string(),
            profile,
            track_counts,
            candidates: draft
                .candidates()
                .iter()
                .map(|candidate| IngestMetadataCandidateInfo::from_candidate(candidate, draft))
                .collect(),
            requirements_configured: draft.review_context().is_some(),
            total_required,
            accepted_required,
            missing_fields,
            complete,
            created_at: value.created_at(),
            updated_at: value.updated_at(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, SimpleObject)]
pub struct IngestMetadataReviewPayload {
    job: IngestJobInfo,
    draft: IngestMetadataDraftInfo,
}

impl From<&IngestMetadataReview> for IngestMetadataReviewPayload {
    fn from(value: &IngestMetadataReview) -> Self {
        Self {
            job: value.job().clone().into(),
            draft: value.metadata().into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, InputObject)]
pub struct ConfigureIngestMetadataReviewInput {
    profile: IngestMetadataProfile,
    track_counts: Vec<i32>,
}

impl TryFrom<ConfigureIngestMetadataReviewInput> for MetadataReviewContext {
    type Error = Error;

    fn try_from(input: ConfigureIngestMetadataReviewInput) -> Result<Self> {
        if input.track_counts.is_empty() || input.track_counts.len() > MAX_DISCS_PER_REVIEW {
            return Err(input_error(
                "INGEST_METADATA_INVALID_LAYOUT",
                format!("trackCounts must contain between 1 and {MAX_DISCS_PER_REVIEW} discs"),
            ));
        }
        let mut total_tracks = 0_u32;
        let mut track_counts = Vec::with_capacity(input.track_counts.len());
        for count in input.track_counts {
            let count = u16::try_from(count)
                .ok()
                .and_then(NonZeroU16::new)
                .ok_or_else(|| {
                    input_error(
                        "INGEST_METADATA_INVALID_LAYOUT",
                        "every track count must be between 1 and 65535",
                    )
                })?;
            total_tracks += u32::from(count.get());
            track_counts.push(count);
        }
        if total_tracks > MAX_TOTAL_TRACKS_PER_REVIEW {
            return Err(input_error(
                "INGEST_METADATA_INVALID_LAYOUT",
                format!("album layout cannot exceed {MAX_TOTAL_TRACKS_PER_REVIEW} tracks"),
            ));
        }
        let layout = AlbumLayout::new(track_counts)
            .map_err(|error| input_error("INGEST_METADATA_INVALID_LAYOUT", error.to_string()))?;
        Ok(Self::new(input.profile.into(), layout))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, InputObject)]
pub struct AddIngestMetadataCandidateInput {
    candidate_id: Option<Uuid>,
    field: IngestMetadataFieldInput,
    value: IngestMetadataValueInput,
    evidence: IngestEvidenceInput,
    confidence_basis_points: i32,
}

impl TryFrom<AddIngestMetadataCandidateInput> for MetadataCandidate {
    type Error = Error;

    fn try_from(input: AddIngestMetadataCandidateInput) -> Result<Self> {
        let confidence = u16::try_from(input.confidence_basis_points)
            .ok()
            .and_then(Confidence::new)
            .ok_or_else(|| {
                input_error(
                    "INGEST_METADATA_INVALID_CONFIDENCE",
                    "confidenceBasisPoints must be between 0 and 10000",
                )
            })?;
        MetadataCandidate::new(
            input.candidate_id.unwrap_or_else(Uuid::new_v4),
            input.field.try_into()?,
            input.value.try_into()?,
            input.evidence.try_into()?,
            confidence,
        )
        .map_err(|error| {
            let code = if matches!(error, MetadataError::EvidenceMethodMismatch { .. }) {
                "INGEST_METADATA_INVALID_EVIDENCE"
            } else {
                "INGEST_METADATA_INVALID_VALUE"
            };
            input_error(code, error.to_string())
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, InputObject)]
pub struct DecideIngestMetadataCandidateInput {
    candidate_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, OneofObject)]
pub enum IngestMetadataEditInput {
    ConfigureReview(ConfigureIngestMetadataReviewInput),
    AddCandidate(AddIngestMetadataCandidateInput),
    AcceptCandidate(DecideIngestMetadataCandidateInput),
    RejectCandidate(DecideIngestMetadataCandidateInput),
}

impl TryFrom<IngestMetadataEditInput> for MetadataEdit {
    type Error = Error;

    fn try_from(input: IngestMetadataEditInput) -> Result<Self> {
        match input {
            IngestMetadataEditInput::ConfigureReview(input) => {
                Ok(Self::ConfigureReview(input.try_into()?))
            }
            IngestMetadataEditInput::AddCandidate(input) => {
                Ok(Self::AddCandidate(input.try_into()?))
            }
            IngestMetadataEditInput::AcceptCandidate(input) => {
                Ok(Self::AcceptCandidate(input.candidate_id))
            }
            IngestMetadataEditInput::RejectCandidate(input) => {
                Ok(Self::RejectCandidate(input.candidate_id))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, InputObject)]
pub struct EditIngestMetadataInput {
    job_id: Uuid,
    expected_row_version: String,
    expected_revision: String,
    edit: IngestMetadataEditInput,
}

#[derive(Debug, Clone, PartialEq, Eq, InputObject)]
pub struct IngestMetadataRevisionActionInput {
    job_id: Uuid,
    expected_row_version: String,
    expected_revision: String,
}

pub async fn query_draft(
    ctx: &Context<'_>,
    job_id: Uuid,
    revision: Option<String>,
) -> Result<Option<IngestMetadataReviewPayload>> {
    let revision = revision.as_deref().map(parse_revision).transpose()?;
    let service = ctx.data::<IngestService>()?;
    service
        .metadata(job_id, revision)
        .await
        .map(|review| review.as_ref().map(Into::into))
        .map_err(service_error)
}

pub async fn query_revisions(
    ctx: &Context<'_>,
    job_id: Uuid,
) -> Result<Vec<IngestMetadataDraftInfo>> {
    let service = ctx.data::<IngestService>()?;
    service
        .metadata_revisions(job_id)
        .await
        .map(|revisions| revisions.iter().map(Into::into).collect())
        .map_err(service_error)
}

pub async fn edit_metadata(
    ctx: &Context<'_>,
    input: EditIngestMetadataInput,
) -> Result<IngestMetadataReviewPayload> {
    let row_version = parse_row_version(&input.expected_row_version)?;
    let revision = parse_revision(&input.expected_revision)?;
    let edit = input.edit.try_into()?;
    let service = ctx.data::<IngestService>()?;
    let review = service
        .edit_metadata(input.job_id, row_version, revision, edit)
        .await
        .map_err(service_error)?;
    Ok((&review).into())
}

pub async fn approve_metadata(
    ctx: &Context<'_>,
    input: IngestMetadataRevisionActionInput,
) -> Result<IngestMetadataReviewPayload> {
    execute_revision_action(ctx, input, RevisionAction::Approve).await
}

pub async fn revise_metadata(
    ctx: &Context<'_>,
    input: IngestMetadataRevisionActionInput,
) -> Result<IngestMetadataReviewPayload> {
    execute_revision_action(ctx, input, RevisionAction::Revise).await
}

#[derive(Debug, Clone, Copy)]
enum RevisionAction {
    Approve,
    Revise,
}

async fn execute_revision_action(
    ctx: &Context<'_>,
    input: IngestMetadataRevisionActionInput,
    action: RevisionAction,
) -> Result<IngestMetadataReviewPayload> {
    let row_version = parse_row_version(&input.expected_row_version)?;
    let revision = parse_revision(&input.expected_revision)?;
    let command = match action {
        RevisionAction::Approve => IngestCommand::ApproveRevision {
            expected_revision: revision,
        },
        RevisionAction::Revise => IngestCommand::ReviseMetadata {
            expected_revision: revision,
        },
    };
    let service = ctx.data::<IngestService>()?;
    service
        .execute(input.job_id, row_version, command)
        .await
        .map_err(service_error)?;
    let review = service
        .metadata(input.job_id, None)
        .await
        .map_err(service_error)?
        .ok_or_else(|| {
            Error::new("committed metadata revision could not be read back")
                .extend_with(|_, extensions| extensions.set("code", "INTERNAL"))
        })?;
    Ok((&review).into())
}

fn parse_position(name: &'static str, value: Option<i32>) -> Result<NonZeroU16> {
    value
        .and_then(|value| u16::try_from(value).ok())
        .and_then(NonZeroU16::new)
        .ok_or_else(|| invalid_field(format!("{name} must be an integer between 1 and 65535")))
}

fn invalid_field(message: impl Into<String>) -> Error {
    input_error("INGEST_METADATA_INVALID_FIELD", message)
}

fn validate_date(date: &AnniDate) -> Result<()> {
    if let Some(month) = date.month() {
        if !(1..=12).contains(&month) {
            return Err(input_error(
                "INGEST_METADATA_INVALID_VALUE",
                "date month must be between 1 and 12",
            ));
        }
        if let Some(day) = date.day()
            && chrono::NaiveDate::from_ymd_opt(
                i32::from(date.year()),
                u32::from(month),
                u32::from(day),
            )
            .is_none()
        {
            return Err(input_error(
                "INGEST_METADATA_INVALID_VALUE",
                "date is not a valid calendar day",
            ));
        }
    }
    Ok(())
}

fn album_field(value: IngestMetadataFieldName) -> Option<AlbumField> {
    Some(match value {
        IngestMetadataFieldName::Title => AlbumField::Title,
        IngestMetadataFieldName::Edition => AlbumField::Edition,
        IngestMetadataFieldName::Artist => AlbumField::Artist,
        IngestMetadataFieldName::Artists => AlbumField::Artists,
        IngestMetadataFieldName::ReleaseDate => AlbumField::ReleaseDate,
        IngestMetadataFieldName::TrackType => AlbumField::TrackType,
        IngestMetadataFieldName::Catalog => AlbumField::Catalog,
        IngestMetadataFieldName::Tags => AlbumField::Tags,
    })
}

fn disc_field(value: IngestMetadataFieldName) -> Option<DiscField> {
    match value {
        IngestMetadataFieldName::Title => Some(DiscField::Title),
        IngestMetadataFieldName::Artist => Some(DiscField::Artist),
        IngestMetadataFieldName::Artists => Some(DiscField::Artists),
        IngestMetadataFieldName::TrackType => Some(DiscField::TrackType),
        IngestMetadataFieldName::Catalog => Some(DiscField::Catalog),
        IngestMetadataFieldName::Tags => Some(DiscField::Tags),
        IngestMetadataFieldName::Edition | IngestMetadataFieldName::ReleaseDate => None,
    }
}

fn track_field(value: IngestMetadataFieldName) -> Option<TrackField> {
    match value {
        IngestMetadataFieldName::Title => Some(TrackField::Title),
        IngestMetadataFieldName::Artist => Some(TrackField::Artist),
        IngestMetadataFieldName::Artists => Some(TrackField::Artists),
        IngestMetadataFieldName::TrackType => Some(TrackField::TrackType),
        IngestMetadataFieldName::Tags => Some(TrackField::Tags),
        IngestMetadataFieldName::Edition
        | IngestMetadataFieldName::ReleaseDate
        | IngestMetadataFieldName::Catalog => None,
    }
}

const fn album_field_name(value: AlbumField) -> IngestMetadataFieldName {
    match value {
        AlbumField::Title => IngestMetadataFieldName::Title,
        AlbumField::Edition => IngestMetadataFieldName::Edition,
        AlbumField::Artist => IngestMetadataFieldName::Artist,
        AlbumField::Artists => IngestMetadataFieldName::Artists,
        AlbumField::ReleaseDate => IngestMetadataFieldName::ReleaseDate,
        AlbumField::TrackType => IngestMetadataFieldName::TrackType,
        AlbumField::Catalog => IngestMetadataFieldName::Catalog,
        AlbumField::Tags => IngestMetadataFieldName::Tags,
    }
}

const fn disc_field_name(value: DiscField) -> IngestMetadataFieldName {
    match value {
        DiscField::Title => IngestMetadataFieldName::Title,
        DiscField::Catalog => IngestMetadataFieldName::Catalog,
        DiscField::Artist => IngestMetadataFieldName::Artist,
        DiscField::Artists => IngestMetadataFieldName::Artists,
        DiscField::TrackType => IngestMetadataFieldName::TrackType,
        DiscField::Tags => IngestMetadataFieldName::Tags,
    }
}

const fn track_field_name(value: TrackField) -> IngestMetadataFieldName {
    match value {
        TrackField::Title => IngestMetadataFieldName::Title,
        TrackField::Artist => IngestMetadataFieldName::Artist,
        TrackField::Artists => IngestMetadataFieldName::Artists,
        TrackField::TrackType => IngestMetadataFieldName::TrackType,
        TrackField::Tags => IngestMetadataFieldName::Tags,
    }
}
