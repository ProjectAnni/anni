use crate::subcommands::workspace::update::WorkspaceUpdateAction;
use anni_common::fs;
use anni_provider::strict_album_path;
use anni_workspace::{AnniWorkspace, WorkspaceAlbumState};
use clap::Args;
use clap_handler::handler;
use std::collections::HashMap;
use std::env::current_dir;
use std::path::PathBuf;

#[derive(Args, Debug, Clone)]
pub struct WorkspacePublishAction {
    // #[clap(long)]
    // copy: bool,
    #[clap(short = 'w', long = "write")]
    write: bool,

    #[clap(short = 'u', long = "uuid")]
    parse_path_as_uuid: bool,

    #[clap(long)]
    soft: bool,

    // publish_to: Option<PathBuf>,
    path: Vec<PathBuf>,
}

#[handler(WorkspacePublishAction)]
pub async fn handle_workspace_publish(mut me: WorkspacePublishAction) -> anyhow::Result<()> {
    let workspace = AnniWorkspace::find(current_dir()?)?;
    let config = workspace.get_config()?;

    let publish_to = config
        .publish_to()
        .expect("Target audio library is not specified in workspace config file.");

    if !publish_to.path.exists() {
        fs::create_dir_all(&publish_to.path)?;
    }

    let map = if me.parse_path_as_uuid {
        let scan_result = workspace.scan()?;
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
    me.path = me
        .path
        .into_iter()
        .filter_map(|path| {
            if me.parse_path_as_uuid {
                let uuid = path.file_name().unwrap().to_str().unwrap();
                let uuid = uuid.parse().expect("Failed to parse uuid");

                let album_path = map.get(&uuid).cloned();
                if album_path.is_none() {
                    warn!("Album with uuid {} is not found in workspace", uuid);
                }

                album_path
            } else {
                Some(path)
            }
        })
        .collect();

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

        let album_id = workspace.get_album_id(&path)?;
        let album_path = workspace.get_album_controlled_path(&album_id)?;

        if let Some(layers) = publish_to.layers {
            // publish as strict
            // 1. get destination path
            let result_path = strict_album_path(&publish_to.path, &album_id.to_string(), layers);
            let result_parent = result_path.parent().expect("Invalid path");
            // 2. create parent directory
            if !result_parent.exists() {
                fs::create_dir_all(&result_parent)?;
            }
            // 3. move/copy album
            if me.soft {
                // copy the whole album
                fs::copy_dir(&album_path, &result_path)?;
                // add soft published mark
                fs::write(album_path.join(".publish"), "")?;
            } else {
                // move directory
                fs::rename(&album_path, &result_path)?;
            }
            // 4. clean album folder
            fs::remove_dir_all(&path, true)?; // TODO: add an option to disable trash feature
        } else {
            // publish as convention
            unimplemented!()
        }
    }
    Ok(())
}
