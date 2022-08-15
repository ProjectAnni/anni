use crate::{ball, ll};
use anni_common::fs;
use anni_flac::blocks::{UserComment, UserCommentExt};
use anni_flac::FlacHeader;
use anni_provider::fs::LocalFileSystemProvider;
use anni_provider::providers::CommonConventionProvider;
use anni_provider::strict_album_path;
use anni_repo::db::RepoDatabaseRead;
use anni_repo::library::{album_info, file_name};
use anni_repo::prelude::*;
use anni_repo::RepositoryManager;
use clap::{Args, Subcommand};
use clap_handler::{handler, Context, Handler};
use std::path::PathBuf;

#[derive(Args, Debug, Clone, Handler)]
#[clap(about = ll!("library"))]
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
    #[clap(name = "tag", alias = "apply")]
    #[clap(about = ll!{"library-tag"})]
    ApplyTag(LibraryApplyTagAction),
    Link(LibraryLinkAction),
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
        .filter_map(|entry| {
            entry
                .metadata()
                .ok()
                .and_then(|meta| if meta.is_dir() { Some(entry) } else { None })
        })
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

fn apply_convention(directory: &PathBuf, album: Album) -> anyhow::Result<()> {
    let discs = album.discs();

    for (disc_num, disc) in album.discs().iter().enumerate() {
        let disc_num = disc_num + 1;
        let disc_dir = if discs.len() > 1 {
            directory.join(format!(
                "[{catalog}] {title} [Disc {disc_num}]",
                catalog = disc.catalog(),
                title = disc.title(),
                disc_num = disc_num,
            ))
        } else {
            directory.to_owned()
        };
        debug!(target: "repo|apply", "Disc dir: {}", disc_dir.to_string_lossy());

        if !disc_dir.exists() {
            bail!("Disc directory does not exist: {:?}", disc_dir);
        }

        let files = fs::get_ext_files(disc_dir, "flac", false)?.unwrap();
        let tracks = disc.tracks();
        if files.len() != tracks.len() {
            bail!(
                "Track number mismatch in Disc {} of {}. Aborted.",
                disc_num,
                album.catalog()
            );
        }

        for (track_num, (file, track)) in files.iter().zip(tracks).enumerate() {
            let track_num = track_num + 1;

            let mut flac = FlacHeader::from_file(file)?;
            let comments = flac.comments();
            // TODO: read anni convention config here
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
                track_total = tracks.len(),
                disc_number = disc_num,
                disc_total = discs.len(),
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
                comments.push(UserComment::track_total(tracks.len()));
                comments.push(UserComment::disc_number(disc_num));
                comments.push(UserComment::disc_total(discs.len()));
                flac.save::<String>(None)?;
            }
        }
    }
    Ok(())
}

#[handler(LibraryApplyTagAction)]
pub fn library_apply_tag(
    me: LibraryApplyTagAction,
    manager: RepositoryManager,
) -> anyhow::Result<()> {
    let manager = manager.into_owned_manager()?;
    for path in me.directories {
        if !path.is_dir() {
            anyhow::bail!("{} is not a directory", path.display());
        }

        let path = path.canonicalize()?;
        let folder_name = path
            .file_name()
            .expect("Failed to get basename of path")
            .to_string_lossy();
        if is_uuid(&folder_name) {
            // strict folder structure, folder name is album_id
            let album = manager
                .albums()
                .get(folder_name.as_ref())
                .ok_or_else(|| anyhow::anyhow!("Album {} not found", folder_name))?;
            apply_strict(&path, album)?;
        } else if let Ok((release_date, catalog, album_title, edition, disc_count)) =
            album_info(&folder_name)
        {
            debug!(target: "repo|apply", "Release date: {}, Catalog: {}, Title: {}", release_date, catalog, album_title);

            // convention folder structure, load album by catalog
            let albums = manager.repo.load_albums(&catalog)?;
            let albums = if albums.len() > 1 {
                albums
                    .into_iter()
                    .filter(|a| a.title_raw() == album_title && a.edition_raw() == edition.as_deref())
                    .collect()
            } else {
                albums
            };
            if albums.is_empty() {
                // no album found
                ball!("repo-album-not-found", catalog = catalog);
            }

            // get track metadata & compare with album folder
            let album = albums.into_iter().nth(0).unwrap();
            if album.title_raw() != album_title
                || album.edition_raw() != edition.as_deref()
                || album.catalog() != catalog
                || album.release_date() != &release_date
            {
                ball!("repo-album-info-mismatch");
            }

            // check discs & tracks
            if album.discs().len() != disc_count {
                bail!("discs.len() != disc_count!");
            }
            apply_convention(&path, album)?;
        } else {
            anyhow::bail!("{} is not a valid album id", folder_name);
        }
    }
    Ok(())
}

fn is_uuid(input: &str) -> bool {
    regex::Regex::new(r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$")
        .unwrap()
        .is_match(input)
}

#[derive(Args, Debug, Clone)]
pub struct LibraryLinkAction {
    #[clap(short, long, default_value = "2")]
    layer: usize,

    #[clap(long)]
    incremental: bool,

    from: PathBuf,
    to: PathBuf,
}

#[handler(LibraryLinkAction)]
pub async fn library_link(me: LibraryLinkAction, manager: RepositoryManager) -> anyhow::Result<()> {
    let manager = manager.into_owned_manager()?;
    let from = me.from.canonicalize()?;
    let to = me.to;
    if !from.is_dir() {
        anyhow::bail!("Must migrate from a directory!");
    }

    if !me.incremental {
        // 1. recreate `to` folder
        fs::remove_dir_all(&to)?; // this function only remove sym link and does not remove the underlying file
        fs::create_dir_all(&to)?;
    }

    // 2. create temp database
    let repo_path = to.join("repo.db");
    manager.to_database(&repo_path)?;

    // 3. scan `from` folder
    let provider = CommonConventionProvider::new(
        from,
        RepoDatabaseRead::new(&repo_path.to_string_lossy().to_string())?,
        Box::new(LocalFileSystemProvider),
    )
    .await?;
    for (album_id, album_from) in provider.albums {
        // 4. create album_id folder
        let album_to = strict_album_path(&to, &album_id, me.layer);
        if me.incremental && album_to.exists() {
            continue;
        }
        fs::create_dir_all(&album_to)?;

        // 5. link album art
        fs::symlink_file(
            album_from.path.join("cover.jpg"),
            album_to.join("cover.jpg"),
        )?;

        let discs = vec![album_from];
        let discs = provider.discs.get(&album_id).unwrap_or(&discs);
        for (i, disc_from) in discs.iter().enumerate() {
            // 6. create disc folder
            let disc_to = album_to.join(format!("{}", i + 1));
            fs::create_dir_all(&disc_to)?;

            // 7. link disc art
            fs::symlink_file(disc_from.path.join("cover.jpg"), disc_to.join("cover.jpg"))?;

            // 8. link tracks
            for entry in fs::read_dir(&disc_from.path)? {
                let entry = entry?;
                let parts = entry.file_name();
                let parts = parts.to_string_lossy();
                let parts: Vec<_> = parts.split('.').collect();
                if let Some(&"flac") = parts.last() {
                    let index = parts.first().unwrap();
                    let track_from = entry.path();
                    let track_to = disc_to.join(format!("{}.flac", index.trim_start_matches('0')));
                    fs::symlink_file(track_from, track_to)?;
                }
            }
        }
    }

    Ok(())
}
