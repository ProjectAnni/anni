mod add;
mod lint;
mod watch;

use crate::args::ActionFile;
use crate::{ball, fl, ll};
use add::*;
use anni_common::fs;
use anni_repo::library::{file_name, AlbumFolderInfo};
use anni_repo::prelude::*;
use anni_repo::{OwnedRepositoryManager, RepositoryManager};
use anni_vgmdb::VGMClient;
use chrono::Datelike;
use clap::{crate_version, ArgEnum, Args, Subcommand};
use clap_handler::{handler, Context, Handler};
use cuna::Cuna;
use lint::*;
use musicbrainz_rs::entity::artist_credit::ArtistCredit;
use musicbrainz_rs::entity::release::Release;
use musicbrainz_rs::Fetch;
use ptree::TreeBuilder;
use std::io::Read;
use std::path::PathBuf;
use std::str::FromStr;
use watch::*;

#[derive(Args, Debug, Clone, Handler)]
#[clap(about = ll!{"repo"})]
#[handler_inject(repo_fields)]
pub struct RepoSubcommand {
    #[clap(long, env = "ANNI_REPO")]
    #[clap(help = ll!{"repo-root"})]
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
    #[clap(about = ll!{"repo-clone"})]
    Clone(RepoCloneAction),
    #[clap(about = ll!{"repo-add"})]
    Add(RepoAddAction),
    #[clap(about = ll!{"repo-import"})]
    Import(RepoImportAction),
    #[clap(about = ll!{"repo-get"})]
    Get(RepoGetAction),
    #[clap(about = ll!{"repo-edit"})]
    Edit(RepoEditAction),
    #[clap(about = ll!{"repo-validate"})]
    #[clap(alias = "validate")]
    Lint(RepoLintAction),
    #[clap(about = ll!{"repo-print"})]
    Print(RepoPrintAction),
    #[clap(name = "db")]
    #[clap(about = ll!{"repo-db"})]
    Database(RepoDatabaseAction),
    // TODO: repo watch
    // Watch(RepoWatchAction),
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

    #[clap(arg_enum)]
    #[clap(short = 'f', long, default_value = "toml")]
    format: RepoImportFormat,

    file: ActionFile,
}

#[derive(ArgEnum, Debug, Clone)]
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
            manager.add_album(album.catalog(), &album, me.allow_duplicate)?;
        }
    }
    Ok(())
}

#[derive(Args, Handler, Debug, Clone)]
pub struct RepoGetAction {
    #[clap(long, global = true)]
    #[clap(help = ll!{"repo-get-print"})]
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
    options: &RepoGetVGMdb,
    manager: &RepositoryManager,
    get: &RepoGetAction,
) -> anyhow::Result<()> {
    let catalog = &options.catalog;

    let album = search_album(&options.keyword.as_deref().unwrap_or(catalog)).await?;

    if get.print {
        println!("{}", album.to_string());
    } else {
        manager.add_album(&options.catalog, &album, false)?;
    }
    Ok(())
}

#[derive(Args, Debug, Clone)]
pub struct RepoGetCue {
    #[clap(short = 'k', long, help = ll!{"repo-get-cue-keyword"})]
    keyword: Option<String>,
    #[clap(short = 'c', long, help = ll!{"repo-get-cue-catalog"})]
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
            Some(catalog) => album.set_catalog(catalog.to_string()),
            None => ball!("repo-cue-insufficient-information"),
        }
    }

    // set artist if performer exists
    let performer = cue.performer().first();
    if let Some(performer) = performer {
        if album.artist().is_empty() {
            album.set_artist(performer.to_string());
        }
    }

    for (file, mut disc) in cue.files().iter().zip(album.iter_mut()) {
        for (cue_track, mut track) in file.tracks.iter().zip(disc.iter_mut()) {
            let performer = cue_track.performer().first();
            track.set_artist(performer.cloned())
        }
    }

    if get.print {
        println!("{}", album.to_string());
    } else {
        let catalog = album.catalog().to_owned();
        manager.add_album(&catalog, &album, false)?;
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
    options: &RepoGetMusicbrainz,
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

    let album = Album::new(
        AlbumInfo {
            title: release.title,
            artist,
            release_date,
            catalog: options.catalog.to_owned(),
            ..Default::default()
        },
        discs,
    );

    if get.print {
        println!("{}", album.to_string());
    } else {
        manager.add_album(&options.catalog, &album, false)?;
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

#[derive(Args, Debug, Clone)]
pub struct RepoPrintAction {
    #[clap(arg_enum)]
    #[clap(short = 't', long = "type", default_value = "title")]
    #[clap(help = ll!("repo-print-type"))]
    print_type: RepoPrintType,

    #[clap(long = "no-generated-by", alias = "no-gb", parse(from_flag = std::ops::Not::not))]
    #[clap(help = ll!("repo-print-clean"))]
    add_generated_by: bool,

    #[clap(help = ll!("repo-print-input"))]
    input: String,

    #[clap(short, long, default_value = "-")]
    #[clap(help = ll!{"export-to"})]
    output: ActionFile,
}

#[handler(RepoPrintAction)]
fn repo_print(me: RepoPrintAction, manager: RepositoryManager) -> anyhow::Result<()> {
    let mut dst = me.output.to_writer()?;

    match me.print_type {
        RepoPrintType::Title
        | RepoPrintType::Artist
        | RepoPrintType::Date
        | RepoPrintType::Cue
        | RepoPrintType::Toml => {
            // print album
            let split: Vec<_> = me.input.split('/').collect();
            let catalog = split[0];
            let disc_id = split
                .get(1)
                .map_or(1, |x| x.parse::<u32>().expect("Invalid disc id"));
            let disc_id = if disc_id > 0 { disc_id - 1 } else { disc_id };

            // FIXME: pick the correct album
            let album = manager.load_albums(catalog)?;
            let album = &album[0];
            match me.print_type {
                RepoPrintType::Title => writeln!(dst, "{}", album.full_title())?,
                RepoPrintType::Artist => writeln!(dst, "{}", album.artist())?,
                RepoPrintType::Date => writeln!(dst, "{}", album.release_date())?,
                RepoPrintType::Cue => match album.iter().nth(disc_id as usize) {
                    Some(disc) => {
                        write!(
                            dst,
                            r#"TITLE "{title}"
PERFORMER "{artist}"
REM DATE "{date}"
"#,
                            title = disc.title(),
                            artist = disc.artist(),
                            date = album.release_date()
                        )?;
                        if me.add_generated_by {
                            write!(
                                dst,
                                r#"REM COMMENT "Generated by Anni v{}""#,
                                crate_version!()
                            )?;
                        }

                        for (track_id, track) in disc.iter().iter().enumerate() {
                            let track_id = track_id + 1;
                            write!(
                                dst,
                                r#"
FILE "{filename}" WAVE
  TRACK 01 AUDIO
    TITLE "{title}"
    PERFORMER "{artist}"
    INDEX 01 00:00:00"#,
                                filename = format!(
                                    "{:02}. {}.flac",
                                    track_id,
                                    track.title().replace("/", "／")
                                ),
                                title = track.title(),
                                artist = track.artist(),
                            )?;
                        }
                    }
                    None => {
                        bail!("Disc {} not found!", disc_id + 1);
                    }
                },
                RepoPrintType::Toml => {
                    write!(dst, "{}", album.to_string())?;
                }
                RepoPrintType::TagTree => unreachable!(),
            }
        }
        RepoPrintType::TagTree => {
            // print tag
            let manager = manager.into_owned_manager()?;

            let tag = TagRef::new(me.input);
            if manager.tag(&tag).is_none() {
                bail!("Tag not found!");
            }

            let mut tree = TreeBuilder::new(tag_to_string(&tag, &manager));
            build_tree(&manager, &tag, &mut tree);
            ptree::print_tree(&tree.build())?;

            fn tag_to_string(tag: &TagRef, manager: &OwnedRepositoryManager) -> String {
                use colored::Colorize;

                let tag_full = manager.tag(tag).unwrap();
                let tag_type = format!("[{:?}]", tag_full.tag_type()).green();
                format!("{tag_type} {}", tag_full.name())
            }

            fn build_tree(manager: &OwnedRepositoryManager, tag: &TagRef, tree: &mut TreeBuilder) {
                let child_tags = manager.child_tags(&tag);
                for tag in child_tags {
                    tree.begin_child(tag_to_string(tag, manager));
                    build_tree(manager, tag, tree);
                    tree.end_child();
                }

                if let Some(albums) = manager.albums_tagged_by(&tag) {
                    for album_id in albums {
                        let album = manager.album(album_id).unwrap();
                        tree.add_empty_child(album.full_title().to_string());
                    }
                }
            }
        }
    }

    Ok(())
}

#[derive(ArgEnum, Debug, PartialEq, Clone)]
pub enum RepoPrintType {
    Title,
    Artist,
    Date,
    Cue,
    Toml,
    TagTree,
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
    #[clap(help = ll!{"export-to"})]
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
