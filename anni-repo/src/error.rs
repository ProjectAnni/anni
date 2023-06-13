use std::path::PathBuf;

use crate::models::TagRef;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("invalid {target} toml: {err:?}\n{input}")]
    TomlParseError {
        target: &'static str,
        input: String,
        err: toml::de::Error,
    },

    #[error("album with the same catalog already exists: {0}")]
    RepoAlbumExists(String),

    #[error("duplicated album: {0}")]
    RepoDuplicatedAlbumId(String),

    #[error("failed to load album {album:?} in repository")]
    RepoAlbumLoadError { album: String },

    #[error("failed to load tags from {file:?}")]
    RepoTagLoadError { file: PathBuf },

    #[error("undefined tags {0:?}")]
    RepoTagsUndefined(Vec<TagRef<'static>>),

    #[error("unknown tag type: {0}")]
    RepoTagUnknownType(String),

    #[error("duplicated tag: {0}")]
    RepoTagDuplicated(TagRef<'static>),

    #[error("repo is locked by another instance")]
    RepoInUse,

    #[error("invalid track type: {0}")]
    InvalidTrackType(String),

    #[error("invalid date: {0}")]
    InvalidDate(String),

    #[error(transparent)]
    IOError(#[from] std::io::Error),

    #[cfg(any(feature = "db-read", feature = "db-write"))]
    #[error(transparent)]
    SqliteError(#[from] rusqlite::Error),

    #[cfg(feature = "db-read")]
    #[error(transparent)]
    SqliteDeserializeError(#[from] serde_rusqlite::Error),

    #[cfg(feature = "git")]
    #[error(transparent)]
    GitError(#[from] git2::Error),

    #[error("multiple errors detected: {0:#?}")]
    MultipleErrors(Vec<Error>),
}

#[cfg(feature = "apply")]
#[derive(thiserror::Error, Debug)]
pub enum AlbumApplyError {
    #[cfg(feature = "apply")]
    #[error("Disc count mismatch when applying album {path}: expected {expected}, got {actual}")]
    DiscMismatch {
        path: PathBuf,
        expected: usize,
        actual: usize,
    },

    #[error("Track count mismatch when applying album {path}: expected {expected}, got {actual}")]
    TrackMismatch {
        path: PathBuf,
        expected: usize,
        actual: usize,
    },

    #[error("Invalid disc folder name {0} got.")]
    InvalidDiscFolder(PathBuf),

    #[error("Missing cover file at: {0}")]
    MissingCover(PathBuf),

    #[error(transparent)]
    IOError(#[from] std::io::Error),

    #[error(transparent)]
    FlacParseError(#[from] anni_flac::error::FlacError),
}
