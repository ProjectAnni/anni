use crate::ll;
use anni_metadata::annim::mutation::add_album::AddAlbumInput;
use anni_metadata::annim::AnnimClient;
use anni_metadata::model::{Album, AlbumInfo, Disc, DiscInfo, UNKNOWN_ARTIST};
use anni_repo::library::{file_name, AlbumFolderInfo};
use anni_repo::models::RepoTrack;
use anni_workspace::{AnniWorkspace, UntrackedWorkspaceAlbum, WorkspaceError};
use clap::Args;
use clap_handler::handler;
use colored::Colorize;
use inquire::Confirm;
use ptree::TreeBuilder;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Args, Debug, Clone)]
pub struct WorkspaceAddAction {
    #[clap(short = 't', long = "tags")]
    #[clap(help = ll!("workspace-add-import-tags"))]
    import_tags: bool,
    #[clap(short = 'd', long)]
    #[clap(help = ll!("workspace-add-dry-run"))]
    dry_run: bool,
    #[clap(short = 'y', long = "yes")]
    #[clap(help = ll!("workspace-add-skip-check"))]
    skip_check: bool,

    #[clap(short = 'e', long = "editor")]
    #[clap(help = ll!("workspace-add-open-editor"))]
    open_editor: bool,

    path: PathBuf,
}

#[handler(WorkspaceAddAction)]
fn handle_workspace_add(me: WorkspaceAddAction) -> anyhow::Result<()> {
    // validate workspace structure
    let workspace = AnniWorkspace::new()?;
    let album_path = me.path;

    let validator = |album: &UntrackedWorkspaceAlbum| -> bool {
        // print album tree
        let album_name = album_path
            .canonicalize()
            .unwrap()
            .file_name()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "Album".to_string());
        let mut tree = TreeBuilder::new(album_name);
        for disc in album.discs.iter() {
            let disc_name = disc
                .path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| {
                    if album.discs.len() > 1 {
                        "Disc 1".to_string()
                    } else {
                        format!("Disc {}", disc.index)
                    }
                });
            let disc_tree = tree.begin_child(disc_name);
            for (index, track) in disc.tracks.iter().enumerate() {
                let index = index + 1;
                let index = format!("[{:02}]", index).green();
                let track_name = track.file_name().unwrap().to_string_lossy();
                disc_tree.add_empty_child(format!("{index} {track_name}"));
            }
            disc_tree.end_child();
        }
        ptree::print_tree(&tree.build()).unwrap();

        // user confirm
        match Confirm::new("Is the album structure correct?")
            .with_default(true)
            .prompt()
        {
            Err(_) | Ok(false) => false,
            _ => true,
        }
    };
    workspace.commit(&album_path, Some(validator))?;

    // import tags if necessary
    if me.import_tags {
        let config = workspace.get_config()?;

        let album_id = workspace.get_album_id(&album_path)?;
        let folder_name = file_name(&album_path)?;

        let album_path = workspace.get_album_controlled_path(&album_id)?;
        let mut discs = Vec::new();
        loop {
            let disc_id = discs.len() + 1;
            let disc_path = album_path.join(disc_id.to_string());
            if !disc_path.exists() {
                break;
            }

            let mut tracks = Vec::new();
            loop {
                let track_id = tracks.len() + 1;
                let track_path = disc_path.join(format!("{track_id}.flac"));
                if !track_path.exists() {
                    break;
                }

                let flac = anni_flac::FlacHeader::from_file(&track_path).map_err(|error| {
                    WorkspaceError::FlacError {
                        path: track_path,
                        error,
                    }
                })?;
                let track: RepoTrack = flac.into();
                tracks.push(track.0)
            }
            discs.push(Disc::new(
                DiscInfo::new(String::new(), None, None, None, None, Default::default()),
                tracks,
            ));
        }

        match config.metadata() {
            anni_workspace::config::WorkspaceMetadata::Repo => {
                // TODO: do not enforce this folder name
                let AlbumFolderInfo {
                    release_date,
                    catalog,
                    title,
                    edition,
                    ..
                } = AlbumFolderInfo::from_str(&folder_name)?;

                for disc in discs.iter_mut() {
                    disc.catalog += &catalog;
                }

                let repo = workspace.to_repository_manager()?;
                let album = Album::new(
                    AlbumInfo {
                        album_id,
                        title: title.to_string(),
                        edition: edition.map(|c| c.to_string()),
                        artist: UNKNOWN_ARTIST.to_string(),
                        release_date,
                        catalog: catalog.to_string(),
                        ..Default::default()
                    },
                    discs,
                );
                repo.add_album(album, false)?;

                if me.open_editor {
                    edit::edit_file(&album_path)?;
                }
            }
            anni_workspace::config::WorkspaceMetadata::Remote { endpoint, token } => {
                let AlbumFolderInfo {
                    release_date,
                    catalog,
                    title,
                    edition,
                    ..
                } = AlbumFolderInfo::from_str(&folder_name)?;

                let client = AnnimClient::new(endpoint, token.as_deref());
                let input = AddAlbumInput {
                    album_id: Some(album_id),
                    title: &title,
                    edition: edition.as_deref(),
                    catalog: Some(catalog.as_ref()),
                    artist: UNKNOWN_ARTIST,
                    year: release_date.year() as i32,
                    month: release_date.month().map(i32::from),
                    day: release_date.day().map(i32::from),
                    extra: None,
                    discs: discs.iter().map(Into::into).collect(),
                };
                client.add_album_input(input, true).await?;
            }
        }
    }

    Ok(())
}
