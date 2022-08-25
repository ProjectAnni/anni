mod init;

use clap_handler::Handler;
use clap::{Args, Subcommand};
use init::WorkspaceInitAction;

#[derive(Args, Handler, Debug, Clone)]
// #[clap(about = ll!("workspace"))]
#[clap(alias = "ws")]
// #[handler_inject(workspace)]
pub struct WorkspaceSubcommand {
    #[clap(subcommand)]
    action: WorkspaceAction,
}

#[derive(Subcommand, Handler, Debug, Clone)]
pub enum WorkspaceAction {
    Init(WorkspaceInitAction),
    // Create,
    // Add,
    // Update,
    // Publish,
}
