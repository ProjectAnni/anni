use crate::workspace::utils::{find_workspace_root, scan_workspace};
use crate::workspace::{WorkspaceAlbum, WorkspaceAlbumState};
use clap::Args;
use clap_handler::handler;
use colored::Colorize;
use inquire::Confirm;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Args, Debug, Clone)]
pub struct WorkspaceStatusAction;

#[handler(WorkspaceStatusAction)]
pub async fn handle_workspace_status() -> anyhow::Result<()> {
    let root = find_workspace_root()?;
    let albums = scan_workspace(&root)?;

    let mut untracked: Vec<(&Path, &Uuid)> = vec![];
    let mut committed: Vec<(&Path, &Uuid)> = vec![];
    let mut dangling: Vec<(&Path, &Uuid)> = vec![];
    let mut garbage: Vec<&Uuid> = vec![];
    for album in albums.iter() {
        match album.state {
            WorkspaceAlbumState::Untracked(ref p) => {
                untracked.push((p.strip_prefix(&root)?, &album.album_id))
            }
            WorkspaceAlbumState::Committed(ref p) => {
                committed.push((p.strip_prefix(&root)?, &album.album_id))
            }
            WorkspaceAlbumState::Dangling(ref p) => {
                dangling.push((p.strip_prefix(&root)?, &album.album_id))
            }
            WorkspaceAlbumState::Garbage => garbage.push(&album.album_id),
        }
    }

    if !untracked.is_empty() {
        println!("Untracked albums:");
        for (path, album_id) in untracked {
            let output = format!("\t[{}]: {}", album_id, path.display()).bright_red();
            println!("{output}");
        }
    }

    if !committed.is_empty() {
        println!("Committed albums:");
        for (path, album_id) in committed {
            let output = format!("\t[{}]: {}", album_id, path.display()).green();
            println!("{output}");
        }
    }

    if !dangling.is_empty() {
        println!("Dangling albums:");
        for (path, album_id) in dangling {
            let output = format!("\t[{}]: {}", album_id, path.display()).red();
            println!("{output}");
        }
    }

    if !garbage.is_empty() {
        println!("Finished albums:");
        for album_id in garbage {
            let output = format!("\t{}", album_id).white();
            println!("{output}");
        }
    }

    Ok(())
}
