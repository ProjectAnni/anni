use crate::ll;
use anni_repo::library::AlbumFolderInfo;
use anni_workspace::{AnniWorkspace, ExtractedAlbumInfo, UntrackedWorkspaceAlbum};
use clap::Args;
use clap_handler::handler;
use colored::Colorize;
use inquire::Confirm;
use ptree::TreeBuilder;
use std::borrow::Cow;
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
        workspace.import_tags(&album_path, |folder_name| {
            let AlbumFolderInfo {
                release_date,
                catalog,
                title,
                edition,
                ..
            } = AlbumFolderInfo::from_str(&folder_name).ok()?;
            Some(ExtractedAlbumInfo {
                title: Cow::Owned(title),
                edition: edition.map(|e| Cow::Owned(e)),
                catalog: Cow::Owned(catalog),
                release_date,
            })
        })?;
    }

    if me.open_editor {
        edit::edit_file(&album_path)?;
    }

    Ok(())
}
