use crate::workspace::utils::{do_get_album_id, find_dot_anni, get_workspace_album_path};
use anni_common::fs::remove_dir_all;
use clap::Args;
use clap_handler::handler;
use inquire::Confirm;
use std::path::PathBuf;

#[derive(Args, Debug, Clone)]
pub struct WorkspaceRmAction {
    #[clap(short = 'y', long = "yes")]
    skip_check: bool,

    path: PathBuf,
}

#[handler(WorkspaceRmAction)]
pub async fn handle_workspace_rm(me: WorkspaceRmAction) -> anyhow::Result<()> {
    let dot_anni = find_dot_anni()?;
    let album_id = do_get_album_id(&me.path)?;
    let album_path =
        get_workspace_album_path(&dot_anni, &album_id).expect("Failed to get album path");

    if !me.skip_check {
        match Confirm::new(&format!("Are you going to remove album {album_id}?"))
            .with_default(false)
            .prompt()
        {
            Err(_) | Ok(false) => bail!("Aborted"),
            _ => {}
        }
    }

    remove_dir_all(me.path, true)?;
    remove_dir_all(&album_path, true)?;
    Ok(())
}
