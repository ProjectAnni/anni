mod add;
mod config;
mod create;
mod fix;
mod init;
mod publish;
mod utils;

use add::*;
use create::*;
use fix::*;
use init::*;
use publish::*;

use crate::ll;
use clap::{Args, Subcommand};
use clap_handler::Handler;

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
    // Update,
    Publish(WorkspacePublishAction),
    Fix(WorkspaceFixAction),
}
