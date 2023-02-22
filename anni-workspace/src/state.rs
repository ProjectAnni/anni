use serde::Serialize;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct WorkspaceAlbum {
    pub album_id: Uuid,
    #[serde(flatten)]
    pub state: WorkspaceAlbumState,
}

pub struct UntrackedWorkspaceAlbum {
    pub album_id: Uuid,
    pub path: PathBuf,
    /// For album with only one disc, the disc directory is not required.
    /// Users can choose to put all tracks in the album directory.
    /// This is called `simplified` album structure.
    pub simplified: bool,
    pub discs: Vec<UntrackedWorkspaceDisc>,
}

pub struct UntrackedWorkspaceDisc {
    pub index: usize,
    pub path: PathBuf,
    pub cover: PathBuf,
    pub tracks: Vec<PathBuf>,
}

/// State of album directory in workspace
#[derive(Debug, Serialize)]
#[serde(tag = "type", content = "path")]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceAlbumState {
    // Normal states
    /// `Untracked` album directory.
    /// Controlled part of the album directory is empty.
    Untracked(PathBuf),
    /// `Committed` album directory.
    /// Controlled part of the album directory is not empty, and User part contains symlinks to the actual file.
    Committed(PathBuf),
    /// `Published` album directory.
    /// Controlled part of the album directory is not empty, and `.publish` file exists.
    Published,

    // Error states
    /// User part of an album exists, but controlled part does not exist, or the symlink is broken.
    Dangling(PathBuf),
    /// User part of an album does not exist, and controlled part is empty.
    Garbage,
}
