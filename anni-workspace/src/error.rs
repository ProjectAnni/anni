use crate::WorkspaceAlbumState;
use anni_repo::error::AlbumApplyError;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(thiserror::Error, Debug)]
pub enum WorkspaceError {
    #[error("Workspace does not exist in given path.")]
    NotAWorkspace,

    #[error("Workspace was not found.")]
    WorkspaceNotFound,

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

    #[error(transparent)]
    RepoError(#[from] anni_repo::error::Error),

    #[error("Invalid flac file {path}: {error}")]
    FlacError {
        path: PathBuf,
        error: anni_flac::error::FlacError,
    },

    // TODO: print full string
    #[error("Failed to extract album info from dir name")]
    FailedToExtractAlbumInfo,

    #[error("Unexpected file {0} found.")]
    UnexpectedFile(PathBuf),

    #[error("Publish target directory {0} was not found.")]
    PublishTargetNotFound(PathBuf),

    #[error(transparent)]
    ApplyError(#[from] AlbumApplyError),
}
