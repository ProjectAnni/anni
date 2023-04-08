use anni_common::fs;
use anni_workspace::{AnniWorkspace, WorkspaceAlbumState};
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
    let workspace = AnniWorkspace::new()?;

    if me.fix_dangling {
        let albums = workspace.scan()?;
        for album in albums {
            if let WorkspaceAlbumState::Dangling(album_path) = album.state {
                let result: anyhow::Result<()> = try {
                    let dot_album = album_path.join(".album");
                    let real_path = workspace.controlled_album_path(&album.album_id, 2);
                    if !real_path.exists() {
                        fs::create_dir_all(&real_path)?;
                    }
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
        let albums = workspace.scan()?;
        for album in albums {
            if let WorkspaceAlbumState::Garbage = album.state {
                let result: anyhow::Result<()> = try {
                    if let Ok(real_path) = workspace.get_album_controlled_path(&album.album_id) {
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
