use anni_common::fs::{self, remove_dir_all};
use anni_workspace::AnniWorkspace;
use clap::Args;
use clap_handler::handler;
use inquire::Confirm;
use std::env::current_dir;
use std::fs::create_dir_all;
use std::path::{Path, PathBuf};

/// Revert workspace album back to uncommitted state
#[derive(Args, Debug, Clone)]
pub struct WorkspaceRmAction {
    #[clap(short = 'y', long = "yes")]
    skip_check: bool,

    path: PathBuf,
}

fn recover_symlinks<P: AsRef<Path>>(path: P) -> anyhow::Result<()> {
    log::debug!("Recovering path: {}", path.as_ref().display());
    let metadata = fs::symlink_metadata(path.as_ref())?;
    if metadata.is_symlink() {
        // ignore .album directories
        if let Some(file_name) = path.as_ref().file_name() {
            if file_name == ".album" {
                return Ok(());
            }
        }

        // copy pointing file to current path
        let actual_path = fs::canonicalize(path.as_ref())?;
        log::debug!("Actual path: {}", actual_path.display());
        fs::rename(actual_path, path)?;
    } else if metadata.is_dir() {
        for entry in path.as_ref().read_dir()? {
            let entry = entry?;
            recover_symlinks(entry.path())?;
        }
    }

    Ok(())
}

#[handler(WorkspaceRmAction)]
pub async fn handle_workspace_rm(me: WorkspaceRmAction) -> anyhow::Result<()> {
    let workspace = AnniWorkspace::find(current_dir()?)?;
    let album_id = workspace.get_album_id(&me.path)?;
    let album_path = workspace
        .get_album_controlled_path(&album_id)
        .expect("Failed to get album path");

    if !me.skip_check {
        match Confirm::new(&format!("Are you going to remove album {album_id}?"))
            .with_default(false)
            .prompt()
        {
            Err(_) | Ok(false) => bail!("Aborted"),
            _ => {}
        }
    }

    recover_symlinks(me.path)?;

    // remove and re-create album path
    remove_dir_all(&album_path, true)?;
    create_dir_all(&album_path)?;
    Ok(())
}
