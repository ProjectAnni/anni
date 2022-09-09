use crate::workspace::utils::{find_dot_anni, get_workspace_album_real_path};
use anni_common::fs;
use clap::{Args, Subcommand};
use clap_handler::{handler, Handler};
use std::path::{Path, PathBuf};

#[derive(Args, Handler, Debug, Clone)]
pub struct WorkspaceFixAction {
    #[clap(subcommand)]
    subcommand: WorkspaceFixSubcommand,
}

#[derive(Subcommand, Handler, Debug, Clone)]
pub enum WorkspaceFixSubcommand {
    Link(WorkspaceFixLinkAction),
    Clean(WorkspaceFixCleanAction),
}

#[derive(Args, Debug, Clone)]
pub struct WorkspaceFixLinkAction {
    path: Vec<PathBuf>,
}

#[handler(WorkspaceFixLinkAction)]
fn handle_workspace_fix_link(me: WorkspaceFixLinkAction) -> anyhow::Result<()> {
    let root = find_dot_anni()?;

    for path in me.path {
        let album_path = path.join(".album");
        let anni_album_path = get_workspace_album_real_path(&root, &path).and_then(|p| {
            if !p.exists() {
                fs::create_dir_all(&p)?;
            }
            Ok(p)
        });

        let result = anni_album_path.and_then(|anni_album_path| {
            // 4. remove .album
            fs::remove_file(&album_path, false)?;

            // 5. relink .album
            fs::symlink_dir(&anni_album_path, &album_path)?;

            Ok(())
        });

        if let Err(e) = result {
            log::error!("Error while fixing album at {}: {}", path.display(), e);
        }
    }

    Ok(())
}

#[derive(Args, Debug, Clone)]
pub struct WorkspaceFixCleanAction;

#[handler(WorkspaceFixCleanAction)]
fn handle_workspace_fix_clean() -> anyhow::Result<()> {
    let root = find_dot_anni()?;

    fn remove_empty_directories<P>(parent: P, level: u8) -> anyhow::Result<()>
    where
        P: AsRef<Path>,
    {
        let parent = parent.as_ref();
        for entry in fs::read_dir(parent)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                if level > 0 {
                    remove_empty_directories(&path, level - 1)?;
                }

                if fs::read_dir(&path)?.next().is_none() {
                    fs::remove_dir(&path)?;
                }
            }
        }
        Ok(())
    }

    // iterate over objects, find empty folders
    remove_empty_directories(&root.join("objects"), 2)?;

    Ok(())
}
