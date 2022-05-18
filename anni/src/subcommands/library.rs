use std::path::PathBuf;
use clap::{Args, Subcommand};
use anni_clap_handler::{Context, Handler, handler};
use anni_common::fs;
use anni_flac::blocks::{UserComment, UserCommentExt};
use anni_flac::FlacHeader;
use anni_repo::library::{album_info, file_name};
use anni_repo::prelude::*;
use anni_repo::RepositoryManager;
use crate::ll;

#[derive(Args, Debug, Clone, Handler)]
#[clap(about = ll ! ("library"))]
#[clap(alias = "lib")]
#[handler_inject(library_fields)]
pub struct LibrarySubcommand {
    #[clap(long = "repo", env = "ANNI_REPO")]
    repo_root: PathBuf,

    #[clap(subcommand)]
    action: LibraryAction,
}

impl LibrarySubcommand {
    async fn library_fields(&self, ctx: &mut Context) -> anyhow::Result<()> {
        let manager = RepositoryManager::new(self.repo_root.as_path())?;
        ctx.insert(manager);
        Ok(())
    }
}


#[derive(Subcommand, Debug, Clone, Handler)]
pub enum LibraryAction {
    New(LibraryNewAlbumAction),
    Tag(LibraryApplyTagAction),
}

#[derive(Args, Debug, Clone)]
pub struct LibraryNewAlbumAction {
    #[clap(short = 'n', long, default_value = "1")]
    disc_num: u8,

    #[clap(default_value = ".")]
    path: PathBuf,
}

#[handler(LibraryNewAlbumAction)]
pub fn library_new_album(me: &LibraryNewAlbumAction) -> anyhow::Result<()> {
    if me.disc_num == 0 {
        anyhow::bail!("disc_num must be > 0");
    }

    let basename = file_name(me.path.as_path())?;
    let album_path = if is_uuid(&basename) {
        me.path.to_path_buf()
    } else {
        let album_id = uuid::Uuid::new_v4().to_string();
        let album_path = me.path.join(album_id);
        fs::create_dir(&album_path)?;
        album_path
    };

    for i in 1..=me.disc_num {
        let disc_path = album_path.join(format!("{}", i));
        fs::create_dir(&disc_path)?;
    }

    Ok(())
}

#[derive(Args, Debug, Clone)]
pub struct LibraryApplyTagAction {
    #[clap(required = true)]
    directories: Vec<PathBuf>,
}

fn apply_strict(directory: &PathBuf, album: &Album) -> anyhow::Result<()> {
    debug!(target: "library|tag", "Directory: {}", directory.display());

    // check disc name
    let mut discs = fs::read_dir(directory)?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| entry.metadata().ok().and_then(|meta| if meta.is_dir() { Some(entry) } else { None }))
        .filter_map(|entry| entry.path().to_str().map(|s| s.to_string()))
        .collect::<Vec<_>>();
    alphanumeric_sort::sort_str_slice(&mut discs);
    if album.discs().len() != discs.len() {
        bail!("discs.len() != discs.len()!");
    }
    for (index, disc_id) in discs.iter().enumerate() {
        let disc_id: usize = disc_id.parse()?;
        if disc_id != index + 1 {
            bail!("disc_id != index + 1!");
        }
    }

    let disc_total = discs.len();

    for ((disc_id, disc), disc_name) in album.discs().iter().enumerate().zip(discs) {
        let disc_num = disc_id + 1;
        let disc_dir = directory.join(disc_name);
        debug!(target: "library|tag", "Disc dir: {}", disc_dir.display());

        let mut files = fs::get_ext_files(disc_dir, "flac", false)?.unwrap();
        alphanumeric_sort::sort_path_slice(&mut files);
        let tracks = disc.tracks();
        let track_total = tracks.len();

        for (track_num, (file, track)) in files.iter().zip(tracks).enumerate() {
            let track_num = track_num + 1;

            let mut flac = FlacHeader::from_file(file)?;
            let comments = flac.comments();
            let meta = format!(
                r#"TITLE={title}
ALBUM={album}
ARTIST={artist}
DATE={release_date}
TRACKNUMBER={track_number}
TRACKTOTAL={track_total}
DISCNUMBER={disc_number}
DISCTOTAL={disc_total}
"#,
                title = track.title(),
                album = disc.title(),
                artist = track.artist(),
                release_date = album.release_date(),
                track_number = track_num,
                disc_number = disc_num,
            );
            // no comment block exist, or comments is not correct
            if comments.is_none() || comments.unwrap().to_string() != meta {
                let comments = flac.comments_mut();
                comments.clear();
                comments.push(UserComment::title(track.title()));
                comments.push(UserComment::album(disc.title()));
                comments.push(UserComment::artist(track.artist()));
                comments.push(UserComment::date(album.release_date()));
                comments.push(UserComment::track_number(track_num));
                comments.push(UserComment::track_total(track_total));
                comments.push(UserComment::disc_number(disc_num));
                comments.push(UserComment::disc_total(disc_total));
                flac.save::<String>(None)?;
            }
        }
    }
    Ok(())
}

#[handler(LibraryApplyTagAction)]
pub fn library_apply_tag(me: LibraryApplyTagAction, manager: RepositoryManager) -> anyhow::Result<()> {
    let manager = manager.into_owned_manager()?;
    for path in me.directories {
        if !path.is_dir() {
            anyhow::bail!("{} is not a directory", path.display());
        }

        let album_id = path.file_name().expect("Failed to get basename of path").to_string_lossy();
        if is_uuid(&album_id) {
            // strict folder structure
            let album = manager.albums().get(album_id.as_ref()).ok_or_else(|| anyhow::anyhow!("Album {} not found", album_id))?;
            apply_strict(&path, album)?;
        } else if let Ok((_date, _catalog, _title, _disc_count)) = album_info(&album_id) {
            // convention folder structure
            todo!()
        } else {
            anyhow::bail!("{} is not a valid album id", album_id);
        }
    }
    Ok(())
}

fn is_uuid(input: &str) -> bool {
    regex::Regex::new(r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$").unwrap().is_match(input)
}