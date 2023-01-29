use std::path::PathBuf;
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum WorkspaceError {
    #[error("Workspace was not found.")]
    WorkspaceNotFound,
    #[error("Workspace detected, but config.toml was not found at {0}")]
    ConfigTomlNotFound(PathBuf),

    #[error("Album with id: {0} exists in workspace")]
    DuplicatedAlbumId(Uuid),

    #[error("Directory is not an album directory: {0}")]
    NotAnAlbum(PathBuf),

    #[error("Album {album_id} has already exist at {path}")]
    AlbumExists { album_id: Uuid, path: PathBuf },

    #[error("Invalid album symlink at: {0}")]
    InvalidAlbumLink(PathBuf),

    #[error("Album not found: {0}")]
    AlbumNotFound(Uuid),

    #[error(transparent)]
    DeserializeError(#[from] toml::de::Error),

    #[error(transparent)]
    InvalidUuid(#[from] uuid::Error),

    #[error(transparent)]
    IOError(#[from] std::io::Error),
}
