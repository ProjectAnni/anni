use crate::models::TagRef;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("invalid {target} toml: {err:?}\n{input}")]
    TomlParseError {
        target: &'static str,
        input: String,
        err: toml_edit::easy::de::Error,
    },

    #[error("album with the same catalog already exists: {0}")]
    RepoAlbumExists(String),

    #[error("duplicated album: {0}")]
    RepoDuplicatedAlbumId(String),

    #[error("failed to load album {album:?} in repository")]
    RepoAlbumLoadError { album: String },

    #[error("failed to load tags from {file:?}")]
    RepoTagLoadError { file: std::path::PathBuf },

    #[error("duplicated tag {tag} defined in {path}")]
    RepoTagDuplicate {
        tag: TagRef<'static>,
        path: std::path::PathBuf,
    },

    #[error("undefined tags {0:?}")]
    RepoTagsUndefined(Vec<TagRef<'static>>),

    #[error("unknown tag type: {0}")]
    RepoTagUnknownType(String),

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
