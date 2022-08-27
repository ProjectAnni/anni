use anni_common::fs;
use anyhow::bail;
use clap::Args;
use clap_handler::handler;
use std::path::PathBuf;
use colored::Colorize;
use inquire::Confirm;
use ptree::TreeBuilder;
use crate::workspace::find_dot_anni;

#[derive(Args, Debug, Clone)]
pub struct WorkspaceAddAction {
    #[clap(short = 't', long)]
    import_tags: bool,
    #[clap(short = 'y', long = "yes")]
    skip_check: bool,

    path: PathBuf,
}

#[handler(WorkspaceAddAction)]
fn handle_workspace_add(me: WorkspaceAddAction) -> anyhow::Result<()> {
    // validate workspace structure
    let _ = find_dot_anni()?;

    // validate album path
    let album_path = me.path.join(".album");
    if !album_path.exists() {
        bail!("Album directory not found at {}", album_path.display());
    }

    // validate album cover
    let album_cover = me.path.join("cover.jpg");
    if !album_cover.exists() {
        bail!("Album cover not found at {}", album_cover.display());
    }

    // iterate over me.path to find all discs
    let mut flac_in_album_root = false;
    let mut discs = vec![];
    for entry in fs::read_dir(&me.path)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_file() {
            if entry.path().extension().unwrap_or_default() == "flac" {
                // flac file in root directory, there should be only one disc
                flac_in_album_root = true;
            }
        } else if file_type.is_dir() {
            // multiple discs exist
            discs.push(entry.path());
        }
    }
    // if there's only one disc, then there should be no sub directories, [true, true]
    // if there are multiple discs, then there should be no flac files in the root directory, [false, false]
    // other conditions are invalid
    if flac_in_album_root ^ discs.is_empty() {
        // both files and discs are empty, or both are not empty
        trace!("flac_in_album_root: {flac_in_album_root}, discs: {:?}", discs);
        bail!("Ambiguous album structure");
    }

    // add album as disc if there's only one disc
    if flac_in_album_root {
        discs.push(me.path.clone());
    }

    // add discs
    struct Disc {
        index: usize,
        path: PathBuf,
        cover: PathBuf,
        tracks: Vec<PathBuf>,
    }
    alphanumeric_sort::sort_path_slice(&mut discs);
    let discs = discs.into_iter().enumerate().map(|(index, disc)| {
        let index = index + 1;

        // iterate over all flac files
        let mut files = fs::read_dir(&disc)?
            .filter_map(|e|
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
            )
            .collect::<Vec<_>>();
        alphanumeric_sort::sort_path_slice(&mut files);

        let disc_cover = disc.join("cover.jpg");
        if !disc_cover.exists() {
            bail!("Disc cover not found in disc {index}!");
        }

        Ok(Disc {
            index,
            path: disc,
            cover: disc_cover,
            tracks: files,
        })
    }).collect::<anyhow::Result<Vec<_>>>()?;

    if !me.skip_check {
        // print album tree
        let album_name = me.path.canonicalize()?.file_name().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| "Album".to_string());
        let mut tree = TreeBuilder::new(album_name);
        for disc in discs.iter() {
            let disc_name = disc.path.file_name()
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
        match Confirm::new("Is the album structure correct?").with_default(true).prompt() {
            Err(_) | Ok(false) => bail!("Aborted"),
            _ => {}
        }
    }

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
    for disc in discs.into_iter() {
        let anni_disc = album_path.join(disc.index.to_string());
        fs::create_dir_all(&anni_disc)?;

        // move tracks
        for (index, track) in disc.tracks.into_iter().enumerate() {
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

    // import tags if necessary
    if me.import_tags {
        // TODO: import tag from strict album directory
        unimplemented!();
    }

    Ok(())
}