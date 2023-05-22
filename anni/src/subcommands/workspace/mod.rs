mod add;
mod create;
mod fsck;
mod init;
mod publish;
mod recover_published;
mod rm;
mod serve;
mod status;
mod target;
mod update;

use add::*;
use create::*;
use fsck::*;
use init::*;
use publish::*;
use rm::*;
use status::*;
use update::*;

use crate::ll;
use crate::subcommands::workspace::recover_published::WorkspaceRecoverPublishedAction;
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
    RecoverPublished(WorkspaceRecoverPublishedAction),
    #[clap(about = ll!("workspace-serve"))]
    Serve(WorkspaceServeAction),
    #[clap(about = ll!("workspace-fsck"))]
    Fsck(WorkspaceFsckAction),
}
