use crate::subcommands::workspace::update::WorkspaceUpdateAction;
use crate::subcommands::workspace::WorkspaceAlbumState;
use crate::workspace::utils::*;
use anni_common::fs;
use anni_provider::strict_album_path;
use anni_repo::library::file_name;
use clap::Args;
use clap_handler::handler;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Args, Debug, Clone)]
pub struct WorkspacePublishAction {
    // #[clap(long)]
    // copy: bool,
    #[clap(short = 'w', long = "write")]
    write: bool,

    #[clap(short = 'u', long = "uuid")]
    parse_path_as_uuid: bool,

    // publish_to: Option<PathBuf>,
    path: Vec<PathBuf>,
}

#[handler(WorkspacePublishAction)]
pub async fn handle_workspace_publish(mut me: WorkspacePublishAction) -> anyhow::Result<()> {
    let root = find_workspace_root()?;
    let dot_anni = find_dot_anni()?;
    let config = super::config::WorkspaceConfig::new(&dot_anni)?;

    let publish_to = config
        .publish_to()
        .expect("Target audio library is not specified in workspace config file.");

    if !publish_to.path.exists() {
        fs::create_dir_all(&publish_to.path)?;
    }

    let map = if me.parse_path_as_uuid {
        let scan_result = scan_workspace(&root)?;
        let mut map = HashMap::new();
        for album in scan_result.into_iter() {
            if let WorkspaceAlbumState::Committed(album_path) = album.state {
                map.insert(album.album_id, album_path);
            }
        }
        map
    } else {
        HashMap::new()
    };
    me.path.iter_mut().for_each(|path| {
        if me.parse_path_as_uuid {
            let uuid = path.file_name().unwrap().to_str().unwrap();
            let uuid = uuid.parse().expect("Failed to parse uuid");
            let album_path = map.get(&uuid).expect("Failed to find album path");
            *path = album_path.clone();
        }
    });

    for path in me.path {
        // validate current path first
        // if normal files exist, abort the operation
        for file in fs::PathWalker::new(&path, true, false, Default::default()) {
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

        if me.write {
            let update = WorkspaceUpdateAction {
                tags: true,
                cover: true,
                path: path.clone(),
            };
            update.run().await?;
        }

        let album_path = get_workspace_album_real_path(&dot_anni, &path)?;
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
