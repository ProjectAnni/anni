use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet, HashSet},
    num::NonZeroU16,
};

use anni_metadata::model::{AnniDate, TrackType};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::MetadataRevision;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlbumField {
    Title,
    Edition,
    Artist,
    Artists,
    ReleaseDate,
    TrackType,
    Catalog,
    Tags,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscField {
    Title,
    Catalog,
    Artist,
    Artists,
    TrackType,
    Tags,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackField {
    Title,
    Artist,
    Artists,
    TrackType,
    Tags,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "scope", content = "location")]
pub enum FieldPath {
    Album(AlbumField),
    Disc {
        disc: NonZeroU16,
        field: DiscField,
    },
    Track {
        disc: NonZeroU16,
        track: NonZeroU16,
        field: TrackField,
    },
}

impl FieldPath {
    pub const fn value_kind(self) -> MetadataValueKind {
        match self {
            Self::Album(AlbumField::Title)
            | Self::Album(AlbumField::Edition)
            | Self::Album(AlbumField::Artist)
            | Self::Album(AlbumField::Catalog)
            | Self::Disc {
                field: DiscField::Title | DiscField::Catalog | DiscField::Artist,
                ..
            }
            | Self::Track {
                field: TrackField::Title | TrackField::Artist,
                ..
            } => MetadataValueKind::Text,
            Self::Album(AlbumField::Artists)
            | Self::Disc {
                field: DiscField::Artists,
                ..
            }
            | Self::Track {
                field: TrackField::Artists,
                ..
            } => MetadataValueKind::TextMap,
            Self::Album(AlbumField::ReleaseDate) => MetadataValueKind::Date,
            Self::Album(AlbumField::TrackType)
            | Self::Disc {
                field: DiscField::TrackType,
                ..
            }
            | Self::Track {
                field: TrackField::TrackType,
                ..
            } => MetadataValueKind::TrackType,
            Self::Album(AlbumField::Tags)
            | Self::Disc {
                field: DiscField::Tags,
                ..
            }
            | Self::Track {
                field: TrackField::Tags,
                ..
            } => MetadataValueKind::TextList,
        }
    }

    const fn allows_absent(self) -> bool {
        matches!(
            self,
            Self::Album(AlbumField::Edition)
                | Self::Disc {
                    field: DiscField::Title
                        | DiscField::Artist
                        | DiscField::Artists
                        | DiscField::TrackType,
                    ..
                }
                | Self::Track {
                    field: TrackField::Artist | TrackField::Artists | TrackField::TrackType,
                    ..
                }
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MetadataValueKind {
    Text,
    Date,
    TrackType,
    TextList,
    TextMap,
    Absent,
}

/// Exact metadata value as observed. Text is never normalized or rewritten.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type", content = "value")]
pub enum MetadataValue {
    Text(String),
    Date(AnniDate),
    TrackType(TrackType),
    TextList(Vec<String>),
    TextMap(BTreeMap<String, String>),
    Absent,
}

impl MetadataValue {
    pub const fn kind(&self) -> MetadataValueKind {
        match self {
            Self::Text(_) => MetadataValueKind::Text,
            Self::Date(_) => MetadataValueKind::Date,
            Self::TrackType(_) => MetadataValueKind::TrackType,
            Self::TextList(_) => MetadataValueKind::TextList,
            Self::TextMap(_) => MetadataValueKind::TextMap,
            Self::Absent => MetadataValueKind::Absent,
        }
    }

    fn is_present(&self) -> bool {
        match self {
            Self::Text(value) => !value.is_empty(),
            Self::TextList(values) => !values.is_empty(),
            Self::TextMap(values) => !values.is_empty(),
            Self::Date(value) => value.year() != 0,
            Self::Absent => false,
            Self::TrackType(_) => true,
        }
    }
}

/// Authority of the underlying source, independent from who collected it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceSourceKind {
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

impl EvidenceSourceKind {
    pub const fn authority_rank(self) -> u8 {
        match self {
            Self::CdBooklet => 0,
            Self::CdPackaging => 1,
            Self::OfficialLabel | Self::OfficialArtist => 2,
            Self::OfficialStore => 3,
            Self::StreamingService => 4,
            Self::Vgmdb => 5,
            Self::CommunitySource => 6,
            Self::Filename => 7,
            Self::DerivedInference => 8,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceMethod {
    ManualTranscription,
    AutomatedExtraction,
    WebImport,
    Inference,
}

impl EvidenceMethod {
    const fn rank(self) -> u8 {
        match self {
            Self::ManualTranscription => 0,
            Self::AutomatedExtraction => 1,
            Self::WebImport => 2,
            Self::Inference => 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    source_kind: EvidenceSourceKind,
    locator: String,
    detail: Option<String>,
    method: EvidenceMethod,
}

impl Evidence {
    pub fn new(
        source_kind: EvidenceSourceKind,
        locator: impl Into<String>,
        detail: Option<String>,
        method: EvidenceMethod,
    ) -> Self {
        Self {
            source_kind,
            locator: locator.into(),
            detail,
            method,
        }
    }

    pub const fn source_kind(&self) -> EvidenceSourceKind {
        self.source_kind
    }

    pub fn locator(&self) -> &str {
        &self.locator
    }

    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }

    pub const fn method(&self) -> EvidenceMethod {
        self.method
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Confidence(u16);

impl Confidence {
    pub const MAX: u16 = 10_000;

    pub const fn new(basis_points: u16) -> Option<Self> {
        if basis_points <= Self::MAX {
            Some(Self(basis_points))
        } else {
            None
        }
    }

    pub const fn basis_points(self) -> u16 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataCandidate {
    id: Uuid,
    field: FieldPath,
    value: MetadataValue,
    evidence: Evidence,
    confidence: Confidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MetadataDecision {
    Pending,
    Accepted,
    Rejected,
}

impl MetadataCandidate {
    pub fn new(
        id: Uuid,
        field: FieldPath,
        value: MetadataValue,
        evidence: Evidence,
        confidence: Confidence,
    ) -> Result<Self, MetadataError> {
        let actual = value.kind();
        let expected = field.value_kind();
        if actual != expected && !(actual == MetadataValueKind::Absent && field.allows_absent()) {
            return Err(MetadataError::ValueKindMismatch {
                field,
                expected,
                actual,
            });
        }
        Ok(Self {
            id,
            field,
            value,
            evidence,
            confidence,
        })
    }

    pub fn track_type_suggestion(
        id: Uuid,
        field: FieldPath,
        title: &str,
        source_candidate_id: Uuid,
        confidence: Confidence,
    ) -> Result<Option<Self>, MetadataError> {
        if !matches!(
            field,
            FieldPath::Track {
                field: TrackField::TrackType,
                ..
            }
        ) {
            return Err(MetadataError::InvalidTrackTypeSuggestionTarget { field });
        }
        let Some(track_type) = TrackType::guess(title) else {
            return Ok(None);
        };
        Ok(Some(Self::new(
            id,
            field,
            MetadataValue::TrackType(track_type),
            Evidence::new(
                EvidenceSourceKind::DerivedInference,
                format!("candidate:{source_candidate_id}"),
                Some("TrackType::guess title heuristic".to_owned()),
                EvidenceMethod::Inference,
            ),
            confidence,
        )?))
    }

    pub const fn id(&self) -> Uuid {
        self.id
    }

    pub const fn field(&self) -> FieldPath {
        self.field
    }

    pub const fn value(&self) -> &MetadataValue {
        &self.value
    }

    pub const fn evidence(&self) -> &Evidence {
        &self.evidence
    }

    pub const fn confidence(&self) -> Confidence {
        self.confidence
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataDraft {
    revision: MetadataRevision,
    review_context: Option<MetadataReviewContext>,
    candidates: Vec<MetadataCandidate>,
    accepted: BTreeMap<FieldPath, Uuid>,
    rejected: HashSet<Uuid>,
}

const METADATA_SNAPSHOT_SCHEMA_VERSION: u16 = 1;

/// Persistence-shaped metadata document.
///
/// Deserializing this type does not make it trusted. Call
/// [`MetadataDraft::restore`] to re-run all domain invariants before use.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataDraftSnapshot {
    schema_version: u16,
    revision: u64,
    #[serde(default)]
    review_context: Option<MetadataReviewContext>,
    candidates: Vec<MetadataCandidateSnapshot>,
    accepted: Vec<AcceptedCandidateSnapshot>,
    rejected: Vec<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct MetadataCandidateSnapshot {
    id: Uuid,
    field: FieldPath,
    value: MetadataValue,
    evidence: Evidence,
    confidence_basis_points: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct AcceptedCandidateSnapshot {
    field: FieldPath,
    candidate_id: Uuid,
}

impl MetadataDraft {
    pub fn new(revision: MetadataRevision) -> Self {
        Self {
            revision,
            review_context: None,
            candidates: Vec::new(),
            accepted: BTreeMap::new(),
            rejected: HashSet::new(),
        }
    }

    pub const fn revision(&self) -> MetadataRevision {
        self.revision
    }

    pub const fn review_context(&self) -> Option<&MetadataReviewContext> {
        self.review_context.as_ref()
    }

    pub fn set_review_context(
        &mut self,
        context: MetadataReviewContext,
    ) -> Result<(), MetadataError> {
        if self
            .review_context
            .as_ref()
            .is_some_and(|current| current != &context)
            && !self.candidates.is_empty()
        {
            return Err(MetadataError::ReviewContextChangeRequiresRevision);
        }
        for candidate in &self.candidates {
            context.validate_field(candidate.field)?;
        }
        self.review_context = Some(context);
        Ok(())
    }

    pub fn candidates(&self) -> &[MetadataCandidate] {
        &self.candidates
    }

    pub fn add_candidate(&mut self, candidate: MetadataCandidate) -> Result<(), MetadataError> {
        if self.candidate(candidate.id).is_some() {
            return Err(MetadataError::DuplicateCandidate { id: candidate.id });
        }
        if let Some(context) = &self.review_context {
            context.validate_field(candidate.field)?;
        }
        self.candidates.push(candidate);
        Ok(())
    }

    pub fn candidate(&self, id: Uuid) -> Option<&MetadataCandidate> {
        self.candidates.iter().find(|candidate| candidate.id == id)
    }

    pub fn recommendation(&self, field: FieldPath) -> Option<&MetadataCandidate> {
        self.candidates
            .iter()
            .filter(|candidate| candidate.field == field && !self.rejected.contains(&candidate.id))
            .min_by(compare_candidates)
    }

    pub fn accept(&mut self, candidate_id: Uuid) -> Result<(), MetadataError> {
        let field = self
            .candidate(candidate_id)
            .ok_or(MetadataError::UnknownCandidate { id: candidate_id })?
            .field;
        self.rejected.remove(&candidate_id);
        self.accepted.insert(field, candidate_id);
        Ok(())
    }

    pub fn reject(&mut self, candidate_id: Uuid) -> Result<(), MetadataError> {
        let field = self
            .candidate(candidate_id)
            .ok_or(MetadataError::UnknownCandidate { id: candidate_id })?
            .field;
        self.rejected.insert(candidate_id);
        if self.accepted.get(&field) == Some(&candidate_id) {
            self.accepted.remove(&field);
        }
        Ok(())
    }

    pub fn accepted_candidate(&self, field: FieldPath) -> Option<&MetadataCandidate> {
        self.accepted
            .get(&field)
            .and_then(|candidate_id| self.candidate(*candidate_id))
    }

    pub fn accepted_value(&self, field: FieldPath) -> Option<&MetadataValue> {
        self.accepted_candidate(field).map(MetadataCandidate::value)
    }

    pub fn decision(&self, candidate_id: Uuid) -> Result<MetadataDecision, MetadataError> {
        let candidate = self
            .candidate(candidate_id)
            .ok_or(MetadataError::UnknownCandidate { id: candidate_id })?;
        if self.accepted.get(&candidate.field) == Some(&candidate_id) {
            Ok(MetadataDecision::Accepted)
        } else if self.rejected.contains(&candidate_id) {
            Ok(MetadataDecision::Rejected)
        } else {
            Ok(MetadataDecision::Pending)
        }
    }

    pub fn fork(&self, revision: MetadataRevision) -> Result<Self, MetadataError> {
        if revision <= self.revision {
            return Err(MetadataError::RevisionNotAdvanced {
                current: self.revision,
                requested: revision,
            });
        }
        let mut next = self.clone();
        next.revision = revision;
        Ok(next)
    }

    pub fn completeness(&self, requirements: &MetadataRequirements) -> CompletenessReport {
        let missing = requirements
            .required
            .iter()
            .copied()
            .filter(|field| {
                !self
                    .accepted_value(*field)
                    .is_some_and(MetadataValue::is_present)
            })
            .collect();
        CompletenessReport {
            total_required: requirements.required.len(),
            missing,
        }
    }

    pub fn review_completeness(&self) -> Result<CompletenessReport, MetadataError> {
        let context = self
            .review_context
            .as_ref()
            .ok_or(MetadataError::ReviewContextMissing)?;
        Ok(self.completeness(&context.requirements()))
    }

    pub fn snapshot(&self) -> MetadataDraftSnapshot {
        let accepted = self
            .accepted
            .iter()
            .map(|(field, candidate_id)| AcceptedCandidateSnapshot {
                field: *field,
                candidate_id: *candidate_id,
            })
            .collect();
        let mut rejected: Vec<_> = self.rejected.iter().copied().collect();
        rejected.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));

        MetadataDraftSnapshot {
            schema_version: METADATA_SNAPSHOT_SCHEMA_VERSION,
            revision: self.revision.get(),
            review_context: self.review_context.clone(),
            candidates: self
                .candidates
                .iter()
                .map(|candidate| MetadataCandidateSnapshot {
                    id: candidate.id,
                    field: candidate.field,
                    value: candidate.value.clone(),
                    evidence: candidate.evidence.clone(),
                    confidence_basis_points: candidate.confidence.basis_points(),
                })
                .collect(),
            accepted,
            rejected,
        }
    }

    pub fn restore(snapshot: MetadataDraftSnapshot) -> Result<Self, MetadataError> {
        if snapshot.schema_version != METADATA_SNAPSHOT_SCHEMA_VERSION {
            return Err(MetadataError::UnsupportedSnapshotVersion {
                actual: snapshot.schema_version,
            });
        }
        let revision = MetadataRevision::new(snapshot.revision).ok_or(
            MetadataError::InvalidSnapshotRevision {
                actual: snapshot.revision,
            },
        )?;
        let mut draft = Self::new(revision);
        if let Some(context) = snapshot.review_context {
            draft.set_review_context(context)?;
        }

        for candidate in snapshot.candidates {
            let confidence = Confidence::new(candidate.confidence_basis_points).ok_or(
                MetadataError::InvalidSnapshotConfidence {
                    candidate_id: candidate.id,
                    actual: candidate.confidence_basis_points,
                },
            )?;
            draft.add_candidate(MetadataCandidate::new(
                candidate.id,
                candidate.field,
                candidate.value,
                candidate.evidence,
                confidence,
            )?)?;
        }

        for candidate_id in snapshot.rejected {
            if !draft.rejected.insert(candidate_id) {
                return Err(MetadataError::DuplicateRejectedCandidate { candidate_id });
            }
            if draft.candidate(candidate_id).is_none() {
                return Err(MetadataError::UnknownCandidate { id: candidate_id });
            }
        }

        for accepted in snapshot.accepted {
            let candidate =
                draft
                    .candidate(accepted.candidate_id)
                    .ok_or(MetadataError::UnknownCandidate {
                        id: accepted.candidate_id,
                    })?;
            if candidate.field != accepted.field {
                return Err(MetadataError::AcceptedFieldMismatch {
                    candidate_id: accepted.candidate_id,
                    declared: accepted.field,
                    actual: candidate.field,
                });
            }
            if draft.rejected.contains(&accepted.candidate_id) {
                return Err(MetadataError::CandidateAcceptedAndRejected {
                    candidate_id: accepted.candidate_id,
                });
            }
            if draft
                .accepted
                .insert(accepted.field, accepted.candidate_id)
                .is_some()
            {
                return Err(MetadataError::DuplicateAcceptedField {
                    field: accepted.field,
                });
            }
        }

        Ok(draft)
    }
}

fn compare_candidates(left: &&MetadataCandidate, right: &&MetadataCandidate) -> Ordering {
    left.evidence
        .source_kind
        .authority_rank()
        .cmp(&right.evidence.source_kind.authority_rank())
        .then_with(|| {
            left.evidence
                .method
                .rank()
                .cmp(&right.evidence.method.rank())
        })
        .then_with(|| right.confidence.cmp(&left.confidence))
        .then_with(|| left.id.as_bytes().cmp(right.id.as_bytes()))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlbumLayout {
    track_counts: Vec<NonZeroU16>,
}

impl AlbumLayout {
    pub fn new(track_counts: Vec<NonZeroU16>) -> Result<Self, MetadataError> {
        if track_counts.is_empty() {
            return Err(MetadataError::EmptyAlbumLayout);
        }
        if track_counts.len() > usize::from(u16::MAX) {
            return Err(MetadataError::TooManyDiscs {
                actual: track_counts.len(),
            });
        }
        Ok(Self { track_counts })
    }

    pub fn track_counts(&self) -> &[NonZeroU16] {
        &self.track_counts
    }

    fn contains(&self, field: FieldPath) -> bool {
        match field {
            FieldPath::Album(_) => true,
            FieldPath::Disc { disc, .. } => {
                self.track_counts.get(usize::from(disc.get() - 1)).is_some()
            }
            FieldPath::Track { disc, track, .. } => self
                .track_counts
                .get(usize::from(disc.get() - 1))
                .is_some_and(|track_count| track.get() <= track_count.get()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetadataProfile {
    Cd,
    Streaming,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataReviewContext {
    profile: MetadataProfile,
    layout: AlbumLayout,
}

impl MetadataReviewContext {
    pub const fn new(profile: MetadataProfile, layout: AlbumLayout) -> Self {
        Self { profile, layout }
    }

    pub const fn profile(&self) -> MetadataProfile {
        self.profile
    }

    pub const fn layout(&self) -> &AlbumLayout {
        &self.layout
    }

    pub fn requirements(&self) -> MetadataRequirements {
        MetadataRequirements::for_review(self.profile, &self.layout)
    }

    fn validate_field(&self, field: FieldPath) -> Result<(), MetadataError> {
        if self.layout.contains(field) {
            Ok(())
        } else {
            Err(MetadataError::FieldOutsideAlbumLayout { field })
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataRequirements {
    required: BTreeSet<FieldPath>,
}

impl MetadataRequirements {
    pub fn for_album(layout: &AlbumLayout) -> Self {
        Self::for_review(MetadataProfile::Cd, layout)
    }

    pub fn for_review(profile: MetadataProfile, layout: &AlbumLayout) -> Self {
        let mut required = BTreeSet::from([
            FieldPath::Album(AlbumField::Title),
            FieldPath::Album(AlbumField::Artist),
            FieldPath::Album(AlbumField::ReleaseDate),
            FieldPath::Album(AlbumField::TrackType),
        ]);
        if profile == MetadataProfile::Cd {
            required.insert(FieldPath::Album(AlbumField::Catalog));
        }
        for (disc_index, track_count) in layout.track_counts.iter().enumerate() {
            let disc = NonZeroU16::new((disc_index + 1) as u16)
                .expect("album layout cannot contain more than u16::MAX discs");
            if profile == MetadataProfile::Cd {
                required.insert(FieldPath::Disc {
                    disc,
                    field: DiscField::Catalog,
                });
            }
            for track in 1..=track_count.get() {
                let track = NonZeroU16::new(track).expect("track number starts at one");
                required.insert(FieldPath::Track {
                    disc,
                    track,
                    field: TrackField::Title,
                });
                required.insert(FieldPath::Track {
                    disc,
                    track,
                    field: TrackField::TrackType,
                });
            }
        }
        Self { required }
    }

    pub fn required(&self) -> &BTreeSet<FieldPath> {
        &self.required
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletenessReport {
    total_required: usize,
    missing: Vec<FieldPath>,
}

impl CompletenessReport {
    pub const fn total_required(&self) -> usize {
        self.total_required
    }

    pub const fn accepted_required(&self) -> usize {
        self.total_required - self.missing.len()
    }

    pub fn missing(&self) -> &[FieldPath] {
        &self.missing
    }

    pub fn is_complete(&self) -> bool {
        self.missing.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MetadataError {
    #[error("metadata field {field:?} expects {expected:?}, got {actual:?}")]
    ValueKindMismatch {
        field: FieldPath,
        expected: MetadataValueKind,
        actual: MetadataValueKind,
    },
    #[error("metadata candidate {id} already exists")]
    DuplicateCandidate { id: Uuid },
    #[error("metadata candidate {id} does not exist")]
    UnknownCandidate { id: Uuid },
    #[error("track type suggestion cannot target {field:?}")]
    InvalidTrackTypeSuggestionTarget { field: FieldPath },
    #[error("metadata revision must advance beyond {current}, got {requested}")]
    RevisionNotAdvanced {
        current: MetadataRevision,
        requested: MetadataRevision,
    },
    #[error("album layout must contain at least one disc")]
    EmptyAlbumLayout,
    #[error("album layout contains {actual} discs, exceeding the u16 limit")]
    TooManyDiscs { actual: usize },
    #[error("metadata review context has not been configured")]
    ReviewContextMissing,
    #[error("metadata review context cannot change after candidates exist; start a new revision")]
    ReviewContextChangeRequiresRevision,
    #[error("metadata field {field:?} falls outside the configured album layout")]
    FieldOutsideAlbumLayout { field: FieldPath },
    #[error("metadata snapshot schema version {actual} is unsupported")]
    UnsupportedSnapshotVersion { actual: u16 },
    #[error("metadata snapshot revision {actual} is invalid")]
    InvalidSnapshotRevision { actual: u64 },
    #[error("metadata candidate {candidate_id} has invalid confidence {actual}; maximum is 10000")]
    InvalidSnapshotConfidence { candidate_id: Uuid, actual: u16 },
    #[error("metadata snapshot rejects candidate {candidate_id} more than once")]
    DuplicateRejectedCandidate { candidate_id: Uuid },
    #[error(
        "metadata snapshot accepts candidate {candidate_id} for {declared:?}, but it belongs to {actual:?}"
    )]
    AcceptedFieldMismatch {
        candidate_id: Uuid,
        declared: FieldPath,
        actual: FieldPath,
    },
    #[error("metadata snapshot both accepts and rejects candidate {candidate_id}")]
    CandidateAcceptedAndRejected { candidate_id: Uuid },
    #[error("metadata snapshot accepts more than one candidate for {field:?}")]
    DuplicateAcceptedField { field: FieldPath },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn confidence(value: u16) -> Confidence {
        Confidence::new(value).unwrap()
    }

    fn evidence(kind: EvidenceSourceKind, locator: &str) -> Evidence {
        Evidence::new(
            kind,
            locator,
            None,
            if kind == EvidenceSourceKind::CdBooklet {
                EvidenceMethod::ManualTranscription
            } else {
                EvidenceMethod::WebImport
            },
        )
    }

    #[test]
    fn accepted_text_preserves_original_unicode_exactly() {
        let value = "曲名（Booklet） / 曲名(Booklet)・A〜B～C";
        let candidate = MetadataCandidate::new(
            Uuid::new_v4(),
            FieldPath::Album(AlbumField::Title),
            MetadataValue::Text(value.to_owned()),
            evidence(EvidenceSourceKind::CdBooklet, "booklet.pdf#page=2"),
            confidence(10_000),
        )
        .unwrap();
        let id = candidate.id();
        let mut draft = MetadataDraft::new(MetadataRevision::INITIAL);
        draft.add_candidate(candidate).unwrap();
        draft.accept(id).unwrap();

        let document = serde_json::to_string(&draft.snapshot()).unwrap();
        let snapshot = serde_json::from_str(&document).unwrap();
        let restored = MetadataDraft::restore(snapshot).unwrap();

        assert_eq!(
            restored.accepted_value(FieldPath::Album(AlbumField::Title)),
            Some(&MetadataValue::Text(value.to_owned()))
        );
        assert_eq!(restored, draft);
    }

    #[test]
    fn booklet_candidate_is_recommended_over_web_and_inference() {
        let field = FieldPath::Album(AlbumField::Title);
        let mut draft = MetadataDraft::new(MetadataRevision::INITIAL);
        for (kind, value, confidence_value) in [
            (
                EvidenceSourceKind::StreamingService,
                "Streaming title",
                10_000,
            ),
            (EvidenceSourceKind::DerivedInference, "AI title", 10_000),
            (EvidenceSourceKind::CdBooklet, "Booklet Title", 8_000),
        ] {
            draft
                .add_candidate(
                    MetadataCandidate::new(
                        Uuid::new_v4(),
                        field,
                        MetadataValue::Text(value.to_owned()),
                        evidence(kind, value),
                        confidence(confidence_value),
                    )
                    .unwrap(),
                )
                .unwrap();
        }

        assert_eq!(
            draft.recommendation(field).unwrap().value(),
            &MetadataValue::Text("Booklet Title".to_owned())
        );
        assert_eq!(draft.accepted_value(field), None);
    }

    #[test]
    fn restore_rejects_corrupt_snapshot_confidence() {
        let candidate_id = Uuid::new_v4();
        let mut draft = MetadataDraft::new(MetadataRevision::INITIAL);
        draft
            .add_candidate(
                MetadataCandidate::new(
                    candidate_id,
                    FieldPath::Album(AlbumField::Title),
                    MetadataValue::Text("Title".to_owned()),
                    evidence(EvidenceSourceKind::CdBooklet, "booklet.pdf#page=2"),
                    confidence(10_000),
                )
                .unwrap(),
            )
            .unwrap();

        let mut snapshot = draft.snapshot();
        snapshot.candidates[0].confidence_basis_points = 10_001;

        assert!(matches!(
            MetadataDraft::restore(snapshot),
            Err(MetadataError::InvalidSnapshotConfidence {
                candidate_id: actual_id,
                actual: 10_001,
            }) if actual_id == candidate_id
        ));
    }

    #[test]
    fn review_context_distinguishes_cd_requirements_and_rejects_unknown_tracks() {
        assert!(!MetadataValue::Date(AnniDate::UNKNOWN).is_present());
        let layout = AlbumLayout::new(vec![NonZeroU16::new(2).unwrap()]).unwrap();
        let cd = MetadataReviewContext::new(MetadataProfile::Cd, layout.clone());
        let streaming = MetadataReviewContext::new(MetadataProfile::Streaming, layout);
        assert!(cd
            .requirements()
            .required()
            .contains(&FieldPath::Album(AlbumField::Catalog)));
        assert!(!streaming
            .requirements()
            .required()
            .contains(&FieldPath::Album(AlbumField::Catalog)));

        let mut draft = MetadataDraft::new(MetadataRevision::INITIAL);
        draft.set_review_context(cd).unwrap();
        let outside_layout = MetadataCandidate::new(
            Uuid::new_v4(),
            FieldPath::Track {
                disc: NonZeroU16::new(1).unwrap(),
                track: NonZeroU16::new(3).unwrap(),
                field: TrackField::Title,
            },
            MetadataValue::Text("Unknown track".to_owned()),
            evidence(EvidenceSourceKind::CdBooklet, "booklet.pdf#page=3"),
            confidence(10_000),
        )
        .unwrap();

        assert!(matches!(
            draft.add_candidate(outside_layout),
            Err(MetadataError::FieldOutsideAlbumLayout { .. })
        ));
    }

    #[test]
    fn inferred_track_type_remains_a_candidate_until_reviewed() {
        let disc = NonZeroU16::new(1).unwrap();
        let track = NonZeroU16::new(1).unwrap();
        let field = FieldPath::Track {
            disc,
            track,
            field: TrackField::TrackType,
        };
        let suggestion = MetadataCandidate::track_type_suggestion(
            Uuid::new_v4(),
            field,
            "Song (Instrumental)",
            Uuid::new_v4(),
            confidence(7_500),
        )
        .unwrap()
        .unwrap();
        let suggestion_id = suggestion.id();
        let mut draft = MetadataDraft::new(MetadataRevision::INITIAL);
        draft.add_candidate(suggestion).unwrap();

        let requirements = MetadataRequirements {
            required: BTreeSet::from([field]),
        };
        assert!(!draft.completeness(&requirements).is_complete());
        draft.accept(suggestion_id).unwrap();
        assert!(draft.completeness(&requirements).is_complete());
        assert_eq!(
            draft.accepted_value(field),
            Some(&MetadataValue::TrackType(TrackType::Instrumental))
        );
    }
}
