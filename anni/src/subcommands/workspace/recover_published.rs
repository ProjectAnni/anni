use anni_common::fs;
use anni_repo::models::DiscRef;
use anni_workspace::AnniWorkspace;
use clap::Args;
use clap_handler::handler;
use std::env::current_dir;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Args, Debug, Clone)]
pub struct WorkspaceRecoverPublishedAction {
    id: Vec<Uuid>,
}

#[handler(WorkspaceRecoverPublishedAction)]
pub async fn handle_workspace_recover_published(
    me: WorkspaceRecoverPublishedAction,
) -> anyhow::Result<()> {
    let workspace = AnniWorkspace::new()?;
    let repo = workspace.to_repository_manager()?.into_owned_manager()?;
    for id in me.id {
        let album = repo
            .album(&id)
            .ok_or_else(|| anyhow::anyhow!("Album {id} not found"))?;
        let album_controlled_path = workspace.controlled_album_path(&id, 2);
        if !album_controlled_path.exists() {
            warn!("Album {id} not found, skipping", id = album.album_id);
            continue;
        }

        let folder_name = format!(
            "[{date}][{catalog}] {title}",
            catalog = album.catalog,
            date = album.release_date().to_short_string(),
            title = album.full_title()
        );

        let album_path = current_dir()?.join(folder_name);
        if album_path.exists() {
            warn!("Album path {} already exists", album_path.display());
            continue;
        }

        // create album folder
        fs::create_dir_all(&album_path)?;
        // create .album
        let dot_album = album_path.join(".album");
        fs::symlink_dir(album_controlled_path, &dot_album)?;

        let total_discs = album.discs_len();

        // create discs
        if total_discs > 1 {
            for index in 0..album.discs_len() {
                let disc_path = album_path.join(format!("Disc {}", index + 1));
                fs::create_dir_all(&disc_path)?;
            }
        }

        fn recover_disc(disc: DiscRef, from_dir: PathBuf, to_dir: PathBuf) -> anyhow::Result<()> {
            for (index, track) in disc.iter().enumerate() {
                let track_from_path = from_dir.join(format!("{}.flac", index + 1));
                let track_to_path =
                    to_dir.join(format!("{:02}. {}.flac", index + 1, track.title()));
                fs::symlink_file(track_from_path, track_to_path)?;
            }

            fs::symlink_file(from_dir.join("cover.jpg"), to_dir.join("cover.jpg"))?;
            Ok(())
        }

        // recover discs
        for (index, disc) in album.iter().enumerate() {
            let disc_target_path = if total_discs > 1 {
                album_path.join(format!("Disc {}", index + 1))
            } else {
                album_path.clone()
            };
            recover_disc(
                disc,
                dot_album.join(format!("{}", index + 1)),
                disc_target_path,
            )?;
        }

        // recover album cover
        let cover_path = dot_album.join("cover.jpg");
        let cover_path_target = album_path.join("cover.jpg");
        if cover_path_target.exists() {
            fs::remove_file(&cover_path_target, false)?;
        }
        fs::symlink_file(cover_path, cover_path_target)?;
    }

    Ok(())
}
