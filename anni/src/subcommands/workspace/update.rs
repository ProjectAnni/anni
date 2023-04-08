use anni_workspace::AnniWorkspace;
use clap::Args;
use clap_handler::handler;
use std::path::PathBuf;

#[derive(Args, Debug, Clone)]
pub struct WorkspaceUpdateAction {
    pub path: PathBuf,
}

#[handler(WorkspaceUpdateAction)]
pub async fn handle_workspace_update(me: WorkspaceUpdateAction) -> anyhow::Result<()> {
    let workspace = AnniWorkspace::new()?;
    workspace.apply_tags(me.path)?;

    Ok(())
}
