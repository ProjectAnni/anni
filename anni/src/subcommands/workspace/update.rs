use anni_workspace::AnniWorkspace;
use clap::Args;
use clap_handler::handler;
use std::env::current_dir;
use std::path::PathBuf;

#[derive(Args, Debug, Clone)]
pub struct WorkspaceUpdateAction {
    pub path: PathBuf,
}

#[handler(WorkspaceUpdateAction)]
pub async fn handle_workspace_update(me: WorkspaceUpdateAction) -> anyhow::Result<()> {
    let workspace = AnniWorkspace::find(current_dir()?)?;
    workspace.apply_tags(me.path)?;

    Ok(())
}
