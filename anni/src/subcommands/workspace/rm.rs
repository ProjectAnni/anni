use anni_workspace::AnniWorkspace;
use clap::Args;
use clap_handler::handler;
use inquire::Confirm;
use std::env::current_dir;
use std::path::PathBuf;

/// Revert workspace album back to uncommitted state
#[derive(Args, Debug, Clone)]
pub struct WorkspaceRmAction {
    #[clap(short = 'y', long = "yes")]
    skip_check: bool,

    path: PathBuf,
}

#[handler(WorkspaceRmAction)]
pub async fn handle_workspace_rm(me: WorkspaceRmAction) -> anyhow::Result<()> {
    let workspace = AnniWorkspace::find(current_dir()?)?;
    let album_id = workspace.get_album_id(&me.path)?;

    if !me.skip_check {
        match Confirm::new(&format!("Are you going to remove album {album_id}?"))
            .with_default(false)
            .prompt()
        {
            Err(_) | Ok(false) => bail!("Aborted"),
            _ => {}
        }
    }

    workspace.revert(me.path)?;
    Ok(())
}
