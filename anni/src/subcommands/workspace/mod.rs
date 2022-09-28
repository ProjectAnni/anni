mod add;
mod config;
mod create;
mod fsck;
mod init;
mod publish;
mod rm;
mod status;
mod target;
mod update;
mod utils;

use add::*;
use create::*;
use fsck::*;
use init::*;
use publish::*;
use rm::*;
use status::*;
use std::path::PathBuf;
use update::*;

use crate::ll;
use clap::{Args, Subcommand};
use clap_handler::Handler;
use uuid::Uuid;

#[derive(Args, Handler, Debug, Clone)]
#[clap(about = ll!("workspace"))]
#[clap(alias = "ws")]
pub struct WorkspaceSubcommand {
    #[clap(subcommand)]
    action: WorkspaceAction,
}

#[derive(Subcommand, Handler, Debug, Clone)]
pub enum WorkspaceAction {
    Init(WorkspaceInitAction),
    Create(WorkspaceCreateAction),
    Add(WorkspaceAddAction),
    Rm(WorkspaceRmAction),
    Status(WorkspaceStatusAction),
    Update(WorkspaceUpdateAction),
    Publish(WorkspacePublishAction),
    Fsck(WorkspaceFsckAction),
}

#[derive(Debug)]
pub struct WorkspaceAlbum {
    pub album_id: Uuid,
    pub state: WorkspaceAlbumState,
}

/// State of album directory in workspace
#[derive(Debug)]
pub enum WorkspaceAlbumState {
    // Normal states
    /// `Untracked` album directory.
    /// Controlled part of the album directory is empty.
    Untracked(PathBuf),
    /// `Committed` album directory.
    /// Controlled part of the album directory is not empty, and User part contains symlinks to the actual file.
    Committed(PathBuf),

    // Error states
    /// User part of an album exists, but controlled part does not exist, or the symlink is broken.
    Dangling(PathBuf),
    /// User part of an album does not exist, and controlled part is empty.
    Garbage,
}
