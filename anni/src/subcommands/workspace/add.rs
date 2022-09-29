use crate::ll;
use crate::workspace::utils::{
    find_dot_anni, get_workspace_album_real_path, get_workspace_repository_manager,
};
use anni_common::fs;
use anni_flac::error::FlacError;
use anni_repo::library::{file_name, AlbumFolderInfo};
use anni_repo::prelude::*;
use anyhow::bail;
use clap::Args;
use clap_handler::handler;
use colored::Colorize;
use inquire::Confirm;
use ptree::TreeBuilder;
use std::path::PathBuf;
use std::str::FromStr;
use uuid::Uuid;

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

    path: PathBuf,
}

#[handler(WorkspaceAddAction)]
fn handle_workspace_add(me: WorkspaceAddAction) -> anyhow::Result<()> {
    // validate workspace structure
    let dot_anni = find_dot_anni()?;

    // validate album path
    let album_path = me.path.join(".album");
    if !album_path.exists() {
        bail!("Album directory not found at {}", album_path.display());
    }

    // check current state of the album
    // whether album_path is empty
    let is_empty = album_path.read_dir()?.next().is_none();
    if !is_empty {
        bail!("Can not add an album that is already committed to workspace.");
    }

    // get album id
    let album_real_path = get_workspace_album_real_path(&dot_anni, &me.path)?;
    let album_id = file_name(&album_real_path)?;
    let album_id = Uuid::from_str(&album_id)?;

    // validate album cover
    let album_cover = me.path.join("cover.jpg");
    if !album_cover.exists() {
        bail!("Album cover not found at {}", album_cover.display());
    }

    // iterate over me.path to find all discs
    let flac_in_album_root = fs::get_ext_file(&me.path, "flac", false)?.is_some();
    let mut discs = fs::get_subdirectories(&me.path)?;

    // if there's only one disc, then there should be no sub directories, [true, true]
    // if there are multiple discs, then there should be no flac files in the root directory, [false, false]
    // other conditions are invalid
    if flac_in_album_root ^ discs.is_empty() {
        // both files and discs are empty, or both are not empty
        trace!(
            "flac_in_album_root: {flac_in_album_root}, discs: {:?}",
            discs
        );
        bail!("Ambiguous album structure");
    }

    // add album as disc if there's only one disc
    if flac_in_album_root {
        discs.push(me.path.clone());
    }

    // add discs
    struct WorkspaceDisc {
        index: usize,
        path: PathBuf,
        cover: PathBuf,
        tracks: Vec<PathBuf>,
    }
    alphanumeric_sort::sort_path_slice(&mut discs);
    let discs = discs
        .into_iter()
        .enumerate()
        .map(|(index, disc)| {
            let index = index + 1;

            // iterate over all flac files
            let mut files = fs::read_dir(&disc)?
                .filter_map(|e| {
                    e.ok().and_then(|e| {
                        let path = e.path();
                        if e.file_type().ok()?.is_file() {
                            if let Some(ext) = path.extension() {
                                if ext == "flac" {
                                    return Some(path);
                                }
                            }
                        }
                        None
                    })
                })
                .collect::<Vec<_>>();
            alphanumeric_sort::sort_path_slice(&mut files);

            let disc_cover = disc.join("cover.jpg");
            if !disc_cover.exists() {
                bail!("Disc cover not found in disc {index}!");
            }

            Ok(WorkspaceDisc {
                index,
                path: disc,
                cover: disc_cover,
                tracks: files,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    if !me.skip_check {
        // print album tree
        let album_name = me
            .path
            .canonicalize()?
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
                    if flac_in_album_root {
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
        ptree::print_tree(&tree.build())?;

        // user confirm
        match Confirm::new("Is the album structure correct?")
            .with_default(true)
            .prompt()
        {
            Err(_) | Ok(false) => bail!("Aborted"),
            _ => {}
        }
    }

    if !me.dry_run {
        ////////////////////////////////// Action Below //////////////////////////////////
        // copy or move album cover
        let anni_album_cover = album_path.join("cover.jpg");
        if flac_in_album_root {
            // cover might be used by discs, copy it
            fs::copy(&album_cover, &anni_album_cover)?;
        } else {
            // move directly
            fs::rename(&album_cover, &anni_album_cover)?;
            fs::symlink_file(&anni_album_cover, &album_cover)?;
        }

        // move discs
        for disc in discs.iter() {
            let anni_disc = album_path.join(disc.index.to_string());
            fs::create_dir_all(&anni_disc)?;

            // move tracks
            for (index, track) in disc.tracks.iter().enumerate() {
                let index = index + 1;
                let anni_track = anni_disc.join(format!("{index}.flac"));
                fs::rename(&track, &anni_track)?;
                fs::symlink_file(&anni_track, &track)?;
            }

            // move disc cover
            let anni_disc_cover = anni_disc.join("cover.jpg");
            fs::rename(&disc.cover, &anni_disc_cover)?;
            fs::symlink_file(&anni_disc_cover, &disc.cover)?;
        }
    }

    // import tags if necessary
    if me.import_tags {
        // import tag from 'strict' album directory
        let repo = get_workspace_repository_manager(&dot_anni)?;
        let folder_name = file_name(&me.path)?;
        let AlbumFolderInfo {
            release_date,
            catalog,
            title,
            edition,
            ..
        } = AlbumFolderInfo::from_str(&folder_name)?;
        let discs: Vec<_> = discs
            .into_iter()
            .map(|disc| {
                let tracks = disc
                    .tracks
                    .into_iter()
                    .map(|track| {
                        let flac = anni_flac::FlacHeader::from_file(&track)?;
                        Ok(flac.into())
                    })
                    .collect::<Result<_, FlacError>>()?;
                Ok(Disc::new(
                    DiscInfo::new(catalog.clone(), None, None, None, Default::default()),
                    tracks,
                ))
            })
            .collect::<Result<_, FlacError>>()?;
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

    Ok(())
}
