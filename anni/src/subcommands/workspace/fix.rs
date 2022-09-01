use crate::workspace::find_dot_anni;
use anni_common::fs;
use anni_provider::strict_album_path;
use clap::{Args, Subcommand};
use clap_handler::{handler, Handler};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Args, Handler, Debug, Clone)]
pub struct WorkspaceFixAction {
    #[clap(subcommand)]
    subcommand: WorkspaceFixSubcommand,
}

#[derive(Subcommand, Handler, Debug, Clone)]
pub enum WorkspaceFixSubcommand {
    Link(WorkspaceFixLinkAction),
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
        let anni_album_path: anyhow::Result<_> = try {
            // 1. find .album
            if !album_path.exists() {
                bail!("Album directory not found at {}", album_path.display());
            }
            if !album_path.is_symlink() {
                bail!("Album directory is not a symlink");
            }

            // 2. get album_id
            let real_path = fs::read_link(&album_path)?;
            let album_id = real_path.file_name().unwrap().to_string_lossy();
            if Uuid::parse_str(&album_id).is_err() {
                bail!("Invalid album id detected");
            }

            // 3. return album_id
            let anni_album_path = strict_album_path(&root.join("objects"), &album_id, 2);
            if !anni_album_path.exists() {
                bail!("Album id {album_id} not found");
            }

            anni_album_path
        };

        let result = anni_album_path.and_then(|anni_album_path| {
            // 4. remove .album
            fs::remove_file(&album_path, false)?;

            // 5. relink .album
            fs::symlink_dir(&anni_album_path, &album_path.join(".album"))?;

            Ok(())
        });

        if let Err(e) = result {
            log::error!("Error while fixing album at {}: {}", path.display(), e);
        }
    }

    Ok(())
}
