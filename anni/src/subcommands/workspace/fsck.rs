use crate::workspace::utils::{
    find_workspace_root, get_workspace_album_path, get_workspace_album_path_or_create,
    scan_workspace,
};
use crate::workspace::WorkspaceAlbumState;
use anni_common::fs;
use clap::Args;
use clap_handler::handler;

#[derive(Args, Debug, Clone)]
pub struct WorkspaceFsckAction {
    #[clap(short = 'd', long)]
    fix_dangling: bool,
    #[clap(long)]
    gc: bool,
}

#[handler(WorkspaceFsckAction)]
fn handle_workspace_fsck(me: WorkspaceFsckAction) -> anyhow::Result<()> {
    let root = find_workspace_root()?;
    let dot_anni = root.join(".anni");

    if me.fix_dangling {
        let albums = scan_workspace(&root)?;
        for album in albums {
            if let WorkspaceAlbumState::Dangling(album_path) = album.state {
                let result: anyhow::Result<()> = try {
                    let dot_album = album_path.join(".album");
                    let real_path = get_workspace_album_path_or_create(&dot_anni, &album.album_id)?;
                    fs::remove_file(&dot_album, false)?;
                    fs::symlink_dir(&real_path, &dot_album)?;
                };

                if let Err(e) = result {
                    log::error!(
                        "Error while fixing album at {}: {}",
                        album_path.display(),
                        e
                    );
                }
            }
        }
    }

    if me.gc {
        let albums = scan_workspace(&root)?;
        for album in albums {
            if let WorkspaceAlbumState::Garbage = album.state {
                let result: anyhow::Result<()> = try {
                    if let Some(real_path) = get_workspace_album_path(&dot_anni, &album.album_id) {
                        // 1. remove garbage album directory
                        fs::remove_dir_all(&real_path, true)?;

                        // 2. try to remove parent
                        if let Some(parent) = real_path.parent() {
                            if parent.read_dir()?.next().is_none() {
                                fs::remove_dir_all(&parent, true)?;

                                // 3. try to remove parent's parent
                                if let Some(parent) = parent.parent() {
                                    if parent.read_dir()?.next().is_none() {
                                        fs::remove_dir_all(&parent, true)?;
                                    }
                                }
                            }
                        }
                    }
                };

                if let Err(e) = result {
                    log::error!("Error while collecting garbage: {}", e);
                }
            }
        }
    }

    Ok(())
}
