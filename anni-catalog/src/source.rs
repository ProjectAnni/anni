use std::{fmt, str::FromStr};

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CatalogSourceKind {
    AppleMusic,
    RecordLabel,
    ArtistWebsite,
    Vgmdb,
    Manual,
}

impl CatalogSourceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AppleMusic => "apple_music",
            Self::RecordLabel => "record_label",
            Self::ArtistWebsite => "artist_website",
            Self::Vgmdb => "vgmdb",
            Self::Manual => "manual",
        }
    }
}

impl fmt::Display for CatalogSourceKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for CatalogSourceKind {
    type Err = UnknownCatalogSourceKind;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "apple_music" => Ok(Self::AppleMusic),
            "record_label" => Ok(Self::RecordLabel),
            "artist_website" => Ok(Self::ArtistWebsite),
            "vgmdb" => Ok(Self::Vgmdb),
            "manual" => Ok(Self::Manual),
            _ => Err(UnknownCatalogSourceKind(value.to_owned())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("unknown catalog source kind: {0}")]
pub struct UnknownCatalogSourceKind(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AcquisitionSourceKind {
    OwnedCd,
    AngelAnime,
    PrivateTracker,
    BitTorrent,
    FriendShare,
    Streaming,
    Other,
}

impl AcquisitionSourceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OwnedCd => "owned_cd",
            Self::AngelAnime => "angel_anime",
            Self::PrivateTracker => "private_tracker",
            Self::BitTorrent => "bit_torrent",
            Self::FriendShare => "friend_share",
            Self::Streaming => "streaming",
            Self::Other => "other",
        }
    }
}

impl fmt::Display for AcquisitionSourceKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for AcquisitionSourceKind {
    type Err = UnknownAcquisitionSourceKind;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "owned_cd" => Ok(Self::OwnedCd),
            "angel_anime" => Ok(Self::AngelAnime),
            "private_tracker" => Ok(Self::PrivateTracker),
            "bit_torrent" => Ok(Self::BitTorrent),
            "friend_share" => Ok(Self::FriendShare),
            "streaming" => Ok(Self::Streaming),
            "other" => Ok(Self::Other),
            _ => Err(UnknownAcquisitionSourceKind(value.to_owned())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("unknown acquisition source kind: {0}")]
pub struct UnknownAcquisitionSourceKind(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoverSourceKind {
    AppleMusic,
    Amazon,
    RecordLabel,
    ArtistWebsite,
    Vgmdb,
    Manual,
}

impl CoverSourceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AppleMusic => "apple_music",
            Self::Amazon => "amazon",
            Self::RecordLabel => "record_label",
            Self::ArtistWebsite => "artist_website",
            Self::Vgmdb => "vgmdb",
            Self::Manual => "manual",
        }
    }
}

impl fmt::Display for CoverSourceKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for CoverSourceKind {
    type Err = UnknownCoverSourceKind;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "apple_music" => Ok(Self::AppleMusic),
            "amazon" => Ok(Self::Amazon),
            "record_label" => Ok(Self::RecordLabel),
            "artist_website" => Ok(Self::ArtistWebsite),
            "vgmdb" => Ok(Self::Vgmdb),
            "manual" => Ok(Self::Manual),
            _ => Err(UnknownCoverSourceKind(value.to_owned())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("unknown cover source kind: {0}")]
pub struct UnknownCoverSourceKind(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyncRunStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

impl SyncRunStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub const fn can_transition_to(self, next: Self) -> bool {
        self as u8 == next as u8
            || matches!(
                (self, next),
                (Self::Queued, Self::Running | Self::Failed | Self::Cancelled)
                    | (
                        Self::Running,
                        Self::Queued | Self::Succeeded | Self::Failed | Self::Cancelled
                    )
            )
    }
}

impl fmt::Display for SyncRunStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for SyncRunStatus {
    type Err = UnknownSyncRunStatus;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "queued" => Ok(Self::Queued),
            "running" => Ok(Self::Running),
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(UnknownSyncRunStatus(value.to_owned())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("unknown sync run status: {0}")]
pub struct UnknownSyncRunStatus(String);

/// How much of a remote catalog a synchronization run is expected to cover.
///
/// This value is intentionally separate from run success. A successful
/// incremental or discovery run is useful evidence, but it must never imply
/// that releases omitted from that response disappeared from the catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyncCoverage {
    FullSnapshot,
    Incremental,
    DiscoveryOnly,
}

impl SyncCoverage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FullSnapshot => "full_snapshot",
            Self::Incremental => "incremental",
            Self::DiscoveryOnly => "discovery_only",
        }
    }

    pub const fn may_infer_absence(self) -> bool {
        matches!(self, Self::FullSnapshot)
    }
}

impl fmt::Display for SyncCoverage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for SyncCoverage {
    type Err = UnknownSyncCoverage;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "full_snapshot" => Ok(Self::FullSnapshot),
            "incremental" => Ok(Self::Incremental),
            "discovery_only" => Ok(Self::DiscoveryOnly),
            _ => Err(UnknownSyncCoverage(value.to_owned())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("unknown sync coverage: {0}")]
pub struct UnknownSyncCoverage(String);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_complete_snapshot_coverage_can_support_absence_inference() {
        for coverage in [
            SyncCoverage::FullSnapshot,
            SyncCoverage::Incremental,
            SyncCoverage::DiscoveryOnly,
        ] {
            assert_eq!(SyncCoverage::from_str(coverage.as_str()), Ok(coverage));
        }
        assert!(SyncCoverage::FullSnapshot.may_infer_absence());
        assert!(!SyncCoverage::Incremental.may_infer_absence());
        assert!(!SyncCoverage::DiscoveryOnly.may_infer_absence());
        assert!(SyncRunStatus::Running.can_transition_to(SyncRunStatus::Queued));
    }
}
