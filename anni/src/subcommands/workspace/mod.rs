mod add;
mod config;
mod create;
mod fsck;
mod init;
mod publish;
mod rm;
mod serve;
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
use update::*;

use crate::ll;
use crate::subcommands::workspace::serve::WorkspaceServeAction;
use clap::{Args, Subcommand};
use clap_handler::Handler;

#[derive(Args, Handler, Debug, Clone)]
#[clap(about = ll!("workspace"))]
#[clap(visible_alias = "ws")]
pub struct WorkspaceSubcommand {
    #[clap(subcommand)]
    action: WorkspaceAction,
}

#[derive(Subcommand, Handler, Debug, Clone)]
pub enum WorkspaceAction {
    #[clap(about = ll!("workspace-init"))]
    Init(WorkspaceInitAction),
    #[clap(about = ll!("workspace-create"))]
    Create(WorkspaceCreateAction),
    #[clap(about = ll!("workspace-add"))]
    Add(WorkspaceAddAction),
    #[clap(about = ll!("workspace-rm"))]
    Rm(WorkspaceRmAction),
    #[clap(about = ll!("workspace-status"))]
    Status(WorkspaceStatusAction),
    #[clap(about = ll!("workspace-update"))]
    Update(WorkspaceUpdateAction),
    #[clap(about = ll!("workspace-publish"))]
    Publish(WorkspacePublishAction),
    #[clap(about = ll!("workspace-serve"))]
    Serve(WorkspaceServeAction),
    #[clap(about = ll!("workspace-fsck"))]
    Fsck(WorkspaceFsckAction),
}

#[derive(Debug, serde::Serialize)]
pub struct WorkspaceAlbum {
    pub album_id: uuid::Uuid,
    #[serde(flatten)]
    pub state: WorkspaceAlbumState,
}

/// State of album directory in workspace
#[derive(Debug, serde::Serialize)]
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
