use crate::workspace::utils::{
    find_dot_anni, get_album_id, get_workspace_album_path, get_workspace_album_real_path,
};
use anni_common::fs;
use clap::Args;
use clap_handler::handler;
use std::path::{Path, PathBuf};

#[derive(Args, Debug, Clone)]
pub struct WorkspaceFsckAction {
    #[clap(short = 'd', long)]
    fix_dangling: bool,
    #[clap(long)]
    gc: bool,
}

#[handler(WorkspaceFsckAction)]
fn handle_workspace_fsck(me: WorkspaceFsckAction) -> anyhow::Result<()> {
    let root = find_dot_anni()?;

    let mut albums_referenced = Vec::new();

    for entry in fs::read_dir(&root)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            // look for .album folder
            match get_album_id(entry.path())? {
                // valid album_id, it's an album directory
                Some(album_id) => {
                    albums_referenced.push(album_id.to_string());
                    let r = get_workspace_album_path(&root, &album_id);
                    // TODO
                }
                // symlink not found, scan recursively
                None => {}
            }
        }
    }

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

    Ok(())
}

// fn handle_workspace_fix_link(me: WorkspaceFixLinkAction) -> anyhow::Result<()> {
//     let root = find_dot_anni()?;
//
//     for path in me.path {
//         let album_path = path.join(".album");
//         let anni_album_path = get_workspace_album_real_path(&root, &path).and_then(|p| {
//             if !p.exists() {
//                 fs::create_dir_all(&p)?;
//             }
//             Ok(p)
//         });
//
//         let result = anni_album_path.and_then(|anni_album_path| {
//             // 4. remove .album
//             fs::remove_file(&album_path, false)?;
//
//             // 5. relink .album
//             fs::symlink_dir(&anni_album_path, &album_path)?;
//
//             Ok(())
//         });
//
//         if let Err(e) = result {
//             log::error!("Error while fixing album at {}: {}", path.display(), e);
//         }
//     }
//
//     Ok(())
// }
//
// #[derive(Args, Debug, Clone)]
// pub struct WorkspaceFixCleanAction;
//
// fn handle_workspace_fix_clean() -> anyhow::Result<()> {
//     let root = find_dot_anni()?;
//
//     fn remove_empty_directories<P>(parent: P, level: u8) -> anyhow::Result<()>
//     where
//         P: AsRef<Path>,
//     {
//         let parent = parent.as_ref();
//         for entry in fs::read_dir(parent)? {
//             let entry = entry?;
//             let path = entry.path();
//             if path.is_dir() {
//                 if level > 0 {
//                     remove_empty_directories(&path, level - 1)?;
//                 }
//
//                 if fs::read_dir(&path)?.next().is_none() {
//                     fs::remove_dir(&path)?;
//                 }
//             }
//         }
//         Ok(())
//     }
//
//     // iterate over objects, find empty folders
//     remove_empty_directories(&root.join("objects"), 2)?;
//
//     Ok(())
// }
