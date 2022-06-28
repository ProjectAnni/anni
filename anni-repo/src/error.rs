use crate::prelude::TagRef;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("invalid {target} toml: {err:?}\n{input}")]
    TomlParseError {
        target: &'static str,
        input: String,
        err: toml::de::Error,
    },

    #[error("failed to initialize repository: {0}")]
    RepoInitError(anyhow::Error),

    #[error("album with the same catalog already exists: {0}")]
    RepoAlbumExists(String),

    #[error("failed to load album {album:?} in repository: {err:?}")]
    RepoAlbumLoadError { album: String, err: anyhow::Error },

    #[error("failed to load tags from {file:?}: {err:?}")]
    RepoTagLoadError {
        file: std::path::PathBuf,
        err: anyhow::Error,
    },

    #[error("duplicated tag {tag} defined in {path}")]
    RepoTagDuplicate {
        tag: TagRef,
        path: std::path::PathBuf,
    },

    #[error("undefined tags {0:?}")]
    RepoTagsUndefined(Vec<TagRef>),

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
}
