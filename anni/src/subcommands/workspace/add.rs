use crate::ll;
use anni_repo::library::{file_name, AlbumFolderInfo};
use anni_repo::prelude::*;
use anni_workspace::{AnniWorkspace, WorkspaceDisc};
use clap::Args;
use clap_handler::handler;
use colored::Colorize;
use inquire::Confirm;
use ptree::TreeBuilder;
use std::env::current_dir;
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
    let workspace = AnniWorkspace::find(current_dir()?)?;
    let album_path = me.path;

    let validator = |discs: &[WorkspaceDisc]| -> bool {
        // print album tree
        let album_name = album_path
            .canonicalize()
            .unwrap()
            .file_name()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "Album".to_string());
        let mut tree = TreeBuilder::new(album_name);
        for disc in discs.iter() {
            let disc_name = disc
                .path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| {
                    if discs.len() > 1 {
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
    let album_id = workspace.commit(&album_path, Some(validator))?;

    // import tags if necessary
    if me.import_tags {
        // import tag from 'strict' album directory
        let repo = workspace.to_repository_manager()?;
        let folder_name = file_name(&album_path)?;
        let AlbumFolderInfo {
            release_date,
            catalog,
            title,
            edition,
            ..
        } = AlbumFolderInfo::from_str(&folder_name)?;
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

                let flac = anni_flac::FlacHeader::from_file(&track_path)?;
                tracks.push(flac.into())
            }
            discs.push(Disc::new(
                DiscInfo::new(catalog.clone(), None, None, None, Default::default()),
                tracks,
            ));
        }
        let album = Album::new(
            AlbumInfo {
                album_id,
                title,
                edition,
                artist: "UnknownArtist".to_string(),
                release_date,
                catalog: catalog.clone(),
                ..Default::default()
            },
            discs,
        );
        repo.add_album(album, false)?;
    }

    if me.open_editor {
        edit::edit_file(&album_path)?;
    }

    Ok(())
}
