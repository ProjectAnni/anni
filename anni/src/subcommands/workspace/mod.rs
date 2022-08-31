mod add;
mod create;
mod init;

use add::*;
use create::*;
use init::*;

use crate::ll;
use clap::{Args, Subcommand};
use clap_handler::Handler;
use std::path::PathBuf;

#[derive(Args, Handler, Debug, Clone)]
#[clap(about = ll ! ("workspace"))]
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
    // Publish,
}

pub fn find_dot_anni() -> anyhow::Result<PathBuf> {
    let path = std::env::current_dir()?;

    let mut path = path.as_path();
    loop {
        let dot_anni = path.join(".anni");
        if dot_anni.exists() {
            return Ok(dot_anni);
        }
        path = path.parent().ok_or_else(|| {
            anyhow::anyhow!("Could not find .anni in current directory or any parent")
        })?;
    }
}
