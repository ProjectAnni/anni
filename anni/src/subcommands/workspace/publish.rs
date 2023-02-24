use crate::subcommands::workspace::update::WorkspaceUpdateAction;
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
        if me.write {
            let update = WorkspaceUpdateAction {
                tags: true,
                cover: true,
                path: path.clone(),
            };
            update.run().await?;

            // TODO: replace with workspace.apply_tags
            // workspace.apply_tags(&path);
        }

        workspace.publish(path, me.soft)?;
    }
    Ok(())
}
