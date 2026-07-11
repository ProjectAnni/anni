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
