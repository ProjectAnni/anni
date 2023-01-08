use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct WorkspaceAlbum {
    pub album_id: Uuid,
    #[serde(flatten)]
    pub state: WorkspaceAlbumState,
}

/// State of album directory in workspace
#[derive(Debug, Serialize)]
#[serde(tag = "type", content = "path")]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceAlbumState {
    // Normal states
    /// `Untracked` album directory.
    /// Controlled part of the album directory is empty.
    Untracked(std::path::PathBuf),
    /// `Committed` album directory.
    /// Controlled part of the album directory is not empty, and User part contains symlinks to the actual file.
    Committed(std::path::PathBuf),

    // Error states
    /// User part of an album exists, but controlled part does not exist, or the symlink is broken.
    Dangling(std::path::PathBuf),
    /// User part of an album does not exist, and controlled part is empty.
    Garbage,
}
