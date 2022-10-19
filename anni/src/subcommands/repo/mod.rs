mod add;
mod lint;
mod print;
mod watch;

use crate::args::ActionFile;
use crate::{ball, fl, ll};
use add::*;
use lint::*;
use print::*;
use watch::*;

use anni_common::fs;
use anni_repo::library::{file_name, AlbumFolderInfo};
use anni_repo::prelude::*;
use anni_repo::RepositoryManager;
use anni_vgmdb::VGMClient;
use chrono::Datelike;
use clap::{Args, Subcommand, ValueEnum};
use clap_handler::{handler, Context, Handler};
use cuna::Cuna;
use musicbrainz_rs::entity::artist_credit::ArtistCredit;
use musicbrainz_rs::entity::release::Release;
use musicbrainz_rs::Fetch;
use std::io::Read;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Args, Debug, Clone, Handler)]
#[clap(about = ll!("repo"))]
#[handler_inject(repo_fields)]
pub struct RepoSubcommand {
    #[clap(long, env = "ANNI_REPO")]
    #[clap(help = ll!("repo-root"))]
    root: PathBuf,

    #[clap(subcommand)]
    action: RepoAction,
}

impl RepoSubcommand {
    async fn repo_fields(&self, ctx: &mut Context) -> anyhow::Result<()> {
        let manager = RepositoryManager::new(self.root.as_path())?;
        ctx.insert(manager);
        Ok(())
    }
}

#[derive(Subcommand, Handler, Debug, Clone)]
pub enum RepoAction {
    #[clap(about = ll!("repo-clone"))]
    Clone(RepoCloneAction),
    #[clap(about = ll!("repo-add"))]
    Add(RepoAddAction),
    #[clap(about = ll!("repo-import"))]
    Import(RepoImportAction),
    #[clap(about = ll!("repo-get"))]
    Get(RepoGetAction),
    #[clap(about = ll!("repo-edit"))]
    Edit(RepoEditAction),
    #[clap(about = ll!("repo-lint"))]
    Lint(RepoLintAction),
    #[clap(about = ll!("repo-print"))]
    Print(RepoPrintAction),
    #[clap(name = "db")]
    #[clap(about = ll!("repo-db"))]
    Database(RepoDatabaseAction),
    Watch(RepoWatchAction),
}

#[derive(Args, Debug, Clone)]
pub struct RepoCloneAction {
    #[clap(required = true)]
    url: String,
    root: Option<PathBuf>,
}

#[handler(RepoCloneAction)]
fn repo_clone(me: RepoCloneAction) -> anyhow::Result<()> {
    let root = me.root.unwrap_or_else(|| PathBuf::from(".")).join("repo");
    log::info!(
        "{}",
        fl!("repo-clone-start", path = root.display().to_string())
    );
    RepositoryManager::clone(&me.url, root)?;
    log::info!("{}", fl!("repo-clone-done"));
    Ok(())
}

#[derive(Args, Debug, Clone)]
pub struct RepoImportAction {
    #[clap(short = 'D', long = "duplicate")]
    allow_duplicate: bool,

    #[clap(value_enum)]
    #[clap(short = 'f', long, default_value = "toml")]
    format: RepoImportFormat,

    file: ActionFile,
}

#[derive(ValueEnum, Debug, Clone)]
pub enum RepoImportFormat {
    // Json,
    Toml,
}

#[handler(RepoImportAction)]
fn repo_import(me: &RepoImportAction, manager: &RepositoryManager) -> anyhow::Result<()> {
    let mut reader = me.file.to_reader()?;
    let mut result = String::new();
    reader.read_to_string(&mut result)?;

    match me.format {
        RepoImportFormat::Toml => {
            let album = Album::from_str(&result)?;
            manager.add_album(album, me.allow_duplicate)?;
        }
    }
    Ok(())
}

#[derive(Args, Handler, Debug, Clone)]
pub struct RepoGetAction {
    #[clap(long, global = true)]
    #[clap(help = ll!("repo-get-print"))]
    print: bool,
    #[clap(subcommand)]
    subcommand: RepoGetSubcommand,
}

#[derive(Subcommand, Handler, Debug, Clone)]
pub enum RepoGetSubcommand {
    #[clap(name = "vgmdb")]
    VGMdb(RepoGetVGMdb),
    #[clap(name = "cue")]
    Cue(RepoGetCue),
    #[clap(name = "musicbrainz")]
    Musicbrainz(RepoGetMusicbrainz),
}

async fn search_album(keyword: &str) -> anyhow::Result<Album> {
    let client = VGMClient::default();
    let search = client.search_albums(keyword).await?;
    let album_got = search.into_album(None).await?;

    let release_date = {
        let split = album_got.release_date().split('-').collect::<Vec<_>>();
        AnniDate::from_parts(
            split[0],
            split.get(1).unwrap_or(&"0"),
            split.get(2).unwrap_or(&"0"),
        )
    };

    let discs = album_got
        .discs
        .iter()
        .map(|disc_got| {
            let disc = DiscInfo::new(
                album_got.catalog().unwrap_or("").to_string(),
                Some(disc_got.title.to_string()),
                None,
                None,
                Default::default(),
            );

            let tracks = disc_got
                .tracks
                .iter()
                .map(|track| {
                    let title = track.get().unwrap().to_string();
                    let track_type = TrackType::guess(&title);
                    TrackInfo::new(title, Some("".to_string()), track_type, Default::default())
                })
                .collect();

            Disc::new(disc, tracks)
        })
        .collect();

    Ok(Album::new(
        AlbumInfo {
            title: album_got.title().unwrap().to_string().into(),
            release_date,
            catalog: album_got.catalog().unwrap_or("").to_string(),
            ..Default::default()
        },
        discs,
    ))
}

#[derive(Args, Debug, Clone)]
pub struct RepoGetVGMdb {
    #[clap(short = 'k', long)]
    keyword: Option<String>,

    catalog: String,
}

#[handler(RepoGetVGMdb)]
fn repo_get_vgmdb(
    options: RepoGetVGMdb,
    manager: &RepositoryManager,
    get: &RepoGetAction,
) -> anyhow::Result<()> {
    let catalog = &options.catalog;

    let mut album = search_album(&options.keyword.as_deref().unwrap_or(catalog)).await?;

    if get.print {
        println!("{}", album.format_to_string());
    } else {
        album.catalog = options.catalog;
        manager.add_album(album, false)?;
    }
    Ok(())
}

#[derive(Args, Debug, Clone)]
pub struct RepoGetCue {
    #[clap(short = 'k', long, help = ll!("repo-get-cue-keyword"))]
    keyword: Option<String>,
    #[clap(short = 'c', long, help = ll!("repo-get-cue-catalog"))]
    catalog: Option<String>,

    path: PathBuf,
}

#[handler(RepoGetCue)]
fn repo_get_cue(
    options: &RepoGetCue,
    manager: &RepositoryManager,
    get: &RepoGetAction,
) -> anyhow::Result<()> {
    let path = &options.path;

    let s = fs::read_to_string(path)?;
    let cue = Cuna::new(&s)?;
    let mut album = match (cue.catalog(), options.keyword.as_ref()) {
        // if catalog is found, fetch metadata from vgmdb
        (Some(catalog), _) => search_album(&catalog.to_string()).await?,
        // otherwise try to search with keyword
        (None, Some(keyword)) => {
            warn!(
                "catalog is unavailable, trying to search vgmdb with keyword `{}`",
                keyword
            );
            search_album(&keyword.to_string()).await?
        }
        // if none is available, try to search with `TITLE` filed in the cue file
        (None, None) => match cue.title().first() {
            Some(title) => {
                warn!("catalog is unavailable, trying to search vgmdb with title `{}`, which may be inaccurate", title);
                search_album(&title.to_string()).await?
            }
            None => ball!("repo-cue-insufficient-information"),
        },
    };

    if album.catalog().is_empty() {
        match &options.catalog {
            Some(catalog) => album.catalog = catalog.to_string(),
            None => ball!("repo-cue-insufficient-information"),
        }
    }

    // set artist if performer exists
    let performer = cue.performer().first();
    if let Some(performer) = performer {
        if album.artist().is_empty() {
            album.artist = performer.to_string();
        }
    }

    for (file, mut disc) in cue.files().iter().zip(album.iter_mut()) {
        for (cue_track, mut track) in file.tracks.iter().zip(disc.iter_mut()) {
            let performer = cue_track.performer().first();
            track.set_artist(performer.cloned())
        }
    }

    if get.print {
        println!("{}", album.format_to_string());
    } else {
        manager.add_album(album, false)?;
    }
    Ok(())
}

#[derive(Args, Debug, Clone)]
pub struct RepoGetMusicbrainz {
    #[clap(long)]
    id: String,
    catalog: String,
}

#[handler(RepoGetMusicbrainz)]
fn repo_get_musicbrainz(
    options: RepoGetMusicbrainz,
    manager: &RepositoryManager,
    get: &RepoGetAction,
) -> anyhow::Result<()> {
    let release = Release::fetch()
        .id(&options.id)
        .with_release_groups()
        .with_recordings()
        .with_artist_credits()
        .execute()?;
    let release_date = release
        .date
        .map(|date| AnniDate::new(date.year() as u16, date.month() as u8, date.day() as u8))
        .unwrap(); // todo: properly deal with unavailable date
    let to_artist = |artists: Vec<ArtistCredit>| {
        artists
            .iter()
            .fold(String::new(), |acc, artist| {
                format!("{}{}、", acc, artist.name)
            })
            .trim_end_matches('、')
            .to_string()
    };
    let artist = release
        .release_group
        .and_then(|rg| rg.artist_credit)
        .map(to_artist)
        .unwrap_or_default();

    let discs = release
        .media
        .into_iter()
        .flatten()
        .map(|media| {
            let disc = DiscInfo::new(
                options.catalog.to_owned(),
                media.title,
                None,
                None,
                Default::default(),
            );

            let tracks = media
                .tracks
                .into_iter()
                .flatten()
                .map(|track| {
                    let track_type = TrackType::guess(&track.title);
                    TrackInfo::new(
                        track.title,
                        track.recording.artist_credit.map(to_artist),
                        track_type,
                        Default::default(),
                    )
                })
                .collect();
            Disc::new(disc, tracks)
        })
        .collect();

    let mut album = Album::new(
        AlbumInfo {
            title: release.title,
            artist,
            release_date,
            catalog: options.catalog,
            ..Default::default()
        },
        discs,
    );

    if get.print {
        println!("{}", album.format_to_string());
    } else {
        manager.add_album(album, false)?;
    }
    Ok(())
}

#[derive(Args, Debug, Clone)]
pub struct RepoEditAction {
    #[clap(required = true)]
    directories: Vec<PathBuf>,
}

#[handler(RepoEditAction)]
fn repo_edit(me: &RepoEditAction, manager: &RepositoryManager) -> anyhow::Result<()> {
    fn do_edit(directory: &PathBuf, manager: &RepositoryManager) -> anyhow::Result<()> {
        let last = file_name(directory)?;
        debug!(target: "repo|edit", "Directory: {}", last);
        if !is_album_folder(&last) {
            ball!("repo-invalid-album", name = last);
        }

        let AlbumFolderInfo { catalog, .. } = AlbumFolderInfo::from_str(&last)?;
        debug!(target: "repo|edit", "Catalog: {}", catalog);
        for file in manager.album_paths(&catalog)? {
            edit::edit_file(&file)?;
        }
        Ok(())
    }

    for directory in me.directories.iter() {
        if let Err(e) = do_edit(directory, manager) {
            error!("{}", e);
        }
    }
    Ok(())
}

fn is_album_folder(input: &str) -> bool {
    let bytes = input.as_bytes();
    let second_last_byte = bytes[bytes.len() - 2];
    !(bytes[bytes.len() - 1] == b']' && second_last_byte > b'0' && second_last_byte < b'9')
}

////////////////////////////////////////////////////////////////////////
// Repo database
#[derive(Args, Debug, Clone)]
pub struct RepoDatabaseAction {
    #[clap(help = ll!("export-to"))]
    output: PathBuf,
}

#[handler(RepoDatabaseAction)]
fn repo_database_action(me: RepoDatabaseAction, manager: RepositoryManager) -> anyhow::Result<()> {
    if !me.output.is_dir() {
        bail!("Output path must be a directory!");
    }

    let manager = manager.into_owned_manager()?;
    manager.to_database(&me.output.join("repo.db"))?;

    Ok(())
}
