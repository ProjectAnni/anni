use crate::WorkspaceAlbumState;
use std::path::PathBuf;
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum WorkspaceError {
    #[error("Workspace was not found.")]
    WorkspaceNotFound,

    #[error("Workspace detected, but config.toml was not found at {0}")]
    ConfigNotFound(PathBuf),

    #[error("Album with id: {0} exists in workspace")]
    DuplicatedAlbumId(Uuid),

    #[error("Directory is not an album directory: {0}")]
    NotAnAlbum(PathBuf),

    #[error("Invalid album state: {0:?}")]
    InvalidAlbumState(WorkspaceAlbumState),

    #[error("Album {album_id} already exists at {path}")]
    AlbumExists { album_id: Uuid, path: PathBuf },

    #[error("Album at {0} was locked")]
    AlbumLocked(PathBuf),

    #[error("Album cover not found at {0}")]
    CoverNotFound(PathBuf),

    #[error("Invalid album symlink at: {0}")]
    InvalidAlbumLink(PathBuf),

    #[error("Album not found: {0}")]
    AlbumNotFound(Uuid),

    #[error("Invalid album found at {0}. If there's only one disc, then subdirectories are not allowed. If there're multiple discs, then having flac files in root directory is unacceptable.")]
    InvalidAlbumDiscStructure(PathBuf),

    #[error("User aborted")]
    UserAborted,

    #[error(transparent)]
    DeserializeError(#[from] toml::de::Error),

    #[error(transparent)]
    InvalidUuid(#[from] uuid::Error),

    #[error(transparent)]
    IOError(#[from] std::io::Error),
}
