use crate::workspace::utils::*;
use anni_common::fs;
use anni_provider::strict_album_path;
use anni_repo::library::file_name;
use clap::Args;
use clap_handler::handler;
use std::path::PathBuf;

#[derive(Args, Debug, Clone)]
pub struct WorkspacePublishAction {
    // #[clap(long)]
    // copy: bool,

    // publish_to: Option<PathBuf>,
    path: Vec<PathBuf>,
}

#[handler(WorkspacePublishAction)]
pub fn handle_workspace_publish(me: WorkspacePublishAction) -> anyhow::Result<()> {
    let root = find_dot_anni()?;
    let config = super::config::WorkspaceConfig::new(&root)?;

    let publish_to = config
        .publish_to()
        .expect("Target audio library is not specified in workspace config file.");

    if !publish_to.path.exists() {
        fs::create_dir_all(&publish_to.path)?;
    }

    for path in me.path {
        // validate current path first
        // if normal files exist, abort the operation
        for file in fs::PathWalker::new(&path, true, false) {
            let file_name = file.file_name().unwrap_or_default();
            if file_name == ".directory" || file_name == ".DS_Store" {
                // skip annoying cases
                continue;
            }

            // if !file.is_symlink() {
            bail!(
                "Regular file {} found in album folder, aborting.",
                file.display()
            );
            // }
        }

        let album_path = get_workspace_album_real_path(&root, &path)?;
        let album_id = file_name(&album_path)?;

        if let Some(layers) = publish_to.layers {
            // publish as strict
            // 1. get destination path
            let result_path = strict_album_path(&publish_to.path, &album_id, layers);
            let result_parent = result_path.parent().expect("Invalid path");
            // 2. create parent directory
            if !result_parent.exists() {
                fs::create_dir_all(&result_parent)?;
            }
            // 3. move album
            fs::rename(&album_path, &result_path)?;
            // 4. clean album folder
            fs::remove_dir_all(&path, true)?; // TODO: add an option to disable trash feature
        } else {
            // publish as convention
            unimplemented!()
        }
    }
    Ok(())
}
