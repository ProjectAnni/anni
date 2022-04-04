use anni_flac::FlacHeader;
use anni_repo::prelude::*;
use anni_repo::library::{album_info, disc_info, file_name};
use anni_repo::RepositoryManager;
use anni_common::fs;
use clap::{Parser, ArgEnum, crate_version};
use crate::{fl, ll, ball};
use std::path::PathBuf;
use std::str::FromStr;
use anni_vgmdb::VGMClient;
use futures::executor::block_on;
use anni_flac::blocks::{UserComment, UserCommentExt};
use anni_clap_handler::{Context, Handler, handler};
use anni_common::inherit::InheritableValue;

#[derive(Parser, Debug, Clone)]
#[clap(about = ll ! {"repo"})]
pub struct RepoSubcommand {
    #[clap(long, env = "ANNI_REPO")]
    #[clap(about = ll ! {"repo-root"})]
    root: PathBuf,

    #[clap(subcommand)]
    action: RepoAction,
}

#[anni_clap_handler::async_trait]
impl Handler for RepoSubcommand {
    async fn handle_command(&mut self, ctx: &mut Context) -> anyhow::Result<()> {
        // Skip manager initialization for migrate subcommands
        if matches!(self.action, RepoAction::Migrate(..)) {
            return Ok(());
        }

        let manager = RepositoryManager::new(self.root.as_path())?;
        ctx.insert(manager);
        Ok(())
    }

    async fn handle_subcommand(&mut self, ctx: Context) -> anyhow::Result<()> {
        self.action.execute(ctx).await
    }
}

#[derive(Parser, Handler, Debug, Clone)]
pub enum RepoAction {
    #[clap(about = ll ! {"repo-clone"})]
    Clone(RepoCloneAction),
    #[clap(about = ll ! {"repo-add"})]
    Add(RepoAddAction),
    #[clap(about = ll ! {"repo-get"})]
    Get(RepoGetAction),
    #[clap(about = ll ! {"repo-edit"})]
    Edit(RepoEditAction),
    #[clap(about = ll ! {"repo-apply"})]
    Apply(RepoApplyAction),
    #[clap(about = ll ! {"repo-validate"})]
    Validate(RepoValidateAction),
    #[clap(about = ll ! {"repo-print"})]
    Print(RepoPrintAction),
    #[clap(name = "db")]
    // TODO: repo-database help message
    Database(RepoDatabaseAction),
    #[clap(about = ll ! {"repo-migrate"})]
    Migrate(RepoMigrateAction),
}

#[derive(Parser, Debug, Clone)]
pub struct RepoCloneAction {
    #[clap(required = true)]
    url: String,
    root: Option<PathBuf>,
}

#[handler(RepoCloneAction)]
fn repo_clone(me: RepoCloneAction) -> anyhow::Result<()> {
    let root = me.root.unwrap_or_else(|| PathBuf::from(".")).join("repo");
    log::info!("{}", fl!("repo-clone-start", path = root.display().to_string()));
    RepositoryManager::clone(&me.url, root)?;
    log::info!("{}", fl!("repo-clone-done"));
    Ok(())
}

#[derive(Parser, Debug, Clone)]
pub struct RepoAddAction {
    #[clap(short = 'e', long)]
    #[clap(about = ll ! ("repo-add-edit"))]
    open_editor: bool,

    #[clap(short = 'D', long = "duplicate")]
    allow_duplicate: bool,

    #[clap(required = true)]
    directories: Vec<PathBuf>,
}

#[handler(RepoAddAction)]
fn repo_add(me: &RepoAddAction, manager: &RepositoryManager) -> anyhow::Result<()> {
    for to_add in me.directories.iter() {
        let last = anni_repo::library::file_name(&to_add)?;
        if !is_album_folder(&last) {
            ball!("repo-invalid-album", name = last);
        }

        let (release_date, catalog, album_title, discs) = album_info(&last)?;
        let mut album = Album::new(album_title.clone(), None, "UnknownArtist".to_string(), release_date, catalog.clone(), Default::default());

        let directories = fs::get_subdirectories(to_add)?;
        let mut directories: Vec<_> = directories.iter().map(|r| r.as_path()).collect();
        if discs == 1 {
            directories.push(to_add);
        }
        if discs != directories.len() {
            bail!("Subdirectory count != disc number!")
        }

        for dir in directories.iter() {
            let files = fs::get_ext_files(PathBuf::from(dir), "flac", false)?.unwrap();
            let mut disc = if discs > 1 {
                let (catalog, disc_title, _) = disc_info(&*file_name(dir)?)?;
                Disc::new(
                    catalog,
                    if album_title != disc_title { Some(disc_title) } else { None },
                    None,
                    None,
                    Default::default(),
                )
            } else {
                Disc::new(catalog.clone(), None, None, None, Default::default())
            };
            for path in files.iter() {
                let header = FlacHeader::from_file(path)?;
                let mut track = stream_to_track(&header);
                // use filename as default track name
                if track.title().is_empty() {
                    track.set_title(file_name(path)?.to_string());
                }

                // auto audio type for instrumental, drama and radio
                let track_type = TrackType::guess(track.title());
                track.set_track_type(track_type);

                disc.push_track(track); // use push_track here to avoid metadata inherit
            }
            disc.fmt(false);
            album.push_disc(disc); // the same
        }
        album.fmt(false);
        album.inherit();

        manager.add_album(&catalog, album, me.allow_duplicate)?;
        if me.open_editor {
            manager.edit_album(&catalog)?;
        }
    }
    Ok(())
}

#[derive(Parser, Handler, Debug, Clone)]
pub struct RepoGetAction {
    // TODO: i18n
    #[clap(long)]
    #[clap(about = ll ! {"repo-get-print"})]
    print: bool,
    #[clap(subcommand)]
    subcommand: RepoGetSubcommand,
}

#[derive(Parser, Handler, Debug, Clone)]
pub enum RepoGetSubcommand {
    #[clap(name = "vgmdb")]
    VGMdb(RepoGetVGMdb),
}

#[derive(Parser, Debug, Clone)]
pub struct RepoGetVGMdb {
    #[clap(short = 'c', long)]
    catalog: String,

    #[clap(short = 'k', long)]
    keyword: Option<String>,
}

#[handler(RepoGetVGMdb)]
fn repo_get_vgmdb(options: &RepoGetVGMdb, manager: &RepositoryManager, get: &RepoGetAction) -> anyhow::Result<()> {
    let catalog = &options.catalog;

    let client = VGMClient::default();
    let search = client.search_albums(&options.keyword.as_deref().unwrap_or(catalog)).await?;
    let album_got = search.get_album(None).await?;

    let date = {
        let split = album_got.release_date.split('-').collect::<Vec<_>>();
        AnniDate::from_parts(split[0], split.get(1).unwrap_or(&"0"), split.get(2).unwrap_or(&"0"))
    };

    let mut album = Album::new(
        album_got.title.get().unwrap().to_string(),
        None,
        Default::default(),
        date,
        album_got.catalog.as_deref().unwrap_or("").to_string(),
        Default::default(),
    );

    for disc_got in album_got.discs {
        let mut disc = Disc::new(
            album_got.catalog.as_deref().unwrap_or("").to_string(),
            Some(disc_got.title.to_string()),
            None,
            None,
            Default::default(),
        );

        for track_got in disc_got.tracks {
            let title = track_got.get().unwrap().to_string();
            let track_type = TrackType::guess(&title);
            disc.push_track(Track::new(
                title,
                InheritableValue::own(String::new()),
                InheritableValue::own(track_type),
                Default::default(),
            ));
        }
        album.push_disc(disc);
    }

    if get.print {
        println!("{}", album.to_string());
    } else {
        manager.add_album(&options.catalog, album, false)?;
    }
    Ok(())
}

#[derive(Parser, Debug, Clone)]
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

        let (_, catalog, _, _) = album_info(&last)?;
        debug!(target: "repo|edit", "Catalog: {}", catalog);
        manager.edit_album(&catalog)?;
        Ok(())
    }

    for directory in me.directories.iter() {
        if let Err(e) = do_edit(directory, manager) {
            error!("{}", e);
        }
    }
    Ok(())
}

#[derive(Parser, Debug, Clone)]
pub struct RepoApplyAction {
    #[clap(required = true)]
    directories: Vec<PathBuf>,
}

#[handler(RepoApplyAction)]
fn repo_apply(me: &RepoApplyAction, manager: &RepositoryManager) -> anyhow::Result<()> {
    fn do_apply(directory: &PathBuf, manager: &RepositoryManager) -> anyhow::Result<()> {
        let last = anni_repo::library::file_name(directory)?;
        debug!(target: "repo|apply", "Directory: {}", last);
        if !is_album_folder(&last) {
            ball!("repo-invalid-album", name = last);
        }

        // extract album info
        let (release_date, catalog, album_title, disc_count) = album_info(&last)?;
        debug!(target: "repo|apply", "Release date: {}, Catalog: {}, Title: {}", release_date, catalog, album_title);

        // load and pick album
        let albums = manager.load_albums(&catalog)?;
        let albums = if albums.len() > 1 {
            albums.into_iter().filter(|a| a.title() == album_title).collect()
        } else { albums };
        if albums.is_empty() {
            // no album found
            ball!("repo-album-not-found", catalog = catalog);
        }

        // get track metadata & compare with album folder
        let album = albums.into_iter().nth(0).unwrap();
        if album.title() != album_title
            || album.catalog() != catalog
            || album.release_date() != &release_date {
            ball!("repo-album-info-mismatch");
        }

        // check discs & tracks
        let discs = album.discs();
        if discs.len() != disc_count {
            bail!("discs.len() != disc_count!");
        }

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
                    catalog
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
                    // TODO: user verify before apply tags
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

    for directory in me.directories.iter() {
        if let Err(e) = do_apply(directory, manager) {
            error!("{}", e)
        }
    }
    Ok(())
}

#[derive(Parser, Debug, Clone)]
pub struct RepoValidateAction {}

#[handler(RepoValidateAction)]
fn repo_validate(manager: RepositoryManager, _: &RepoValidateAction) -> anyhow::Result<()> {
    let mut has_error = false;
    info!(target: "anni", "{}", fl!("repo-validate-start"));

    // initialize owned manager
    let manager = manager.into_owned_manager()?;

    // check albums
    for album in manager.albums_iter() {
        let catalog = album.catalog();

        if album.artist() == "[Unknown Artist]" || album.artist() == "UnknownArtist" {
            error!(target: &format!("repo|{}", catalog), "{}", fl!("repo-invalid-artist", artist = album.artist()));
            has_error = true;
        }
        if let TrackType::Other(o) = album.track_type() {
            warn!(target: &format!("repo|{}", catalog), "Unknown track type: {}", o);
        }

        for (disc_id, disc) in album.discs().iter().enumerate() {
            let disc_id = disc_id + 1;
            if let TrackType::Other(o) = disc.track_type() {
                warn!(target: &format!("repo|{}", catalog), "Unknown track type in disc {}: {}", disc_id, o);
            }

            for (track_id, track) in disc.tracks().iter().enumerate() {
                let track_id = track_id + 1;
                if let TrackType::Other(o) = track.track_type() {
                    warn!(target: &format!("repo|{}", catalog), "Unknown track type in disc {} track {}: {}", disc_id, track_id, o);
                }
            }
        }
    }
    // check tags
    if let Some(path) = manager.check_tags_loop() {
        log::error!(target: "repo|tags", "Loop detected: {:?}", path);
        has_error = true;
    }
    if !has_error {
        info!(target: "anni", "{}", fl!("repo-validate-end"));
        Ok(())
    } else {
        ball!("repo-validate-failed");
    }
}

#[derive(Parser, Debug, Clone)]
pub struct RepoPrintAction {
    #[clap(arg_enum)]
    #[clap(short = 't', long = "type", default_value = "title")]
    #[clap(about = ll ! ("repo-print-type"))]
    print_type: RepoPrintType,

    #[clap(long = "no-generated-by", alias = "no-gb", parse(from_flag = std::ops::Not::not))]
    #[clap(about = ll ! ("repo-print-clean"))]
    add_generated_by: bool,

    #[clap(about = ll ! ("repo-print-catalog"))]
    catalog: String,

    #[clap(short, long, default_value = "-")]
    #[clap(about = ll ! {"export-to"})]
    output: crate::args::ActionFile,
}

#[handler(RepoPrintAction)]
fn repo_print(me: &RepoPrintAction, manager: &RepositoryManager) -> anyhow::Result<()> {
    let split: Vec<_> = me.catalog.split('/').collect();
    let (catalog, disc_id) = if split.len() == 1 {
        (split[0], 1)
    } else {
        (split[0], usize::from_str(split[1]).expect("Invalid disc id"))
    };
    let disc_id = if disc_id > 0 { disc_id - 1 } else { disc_id };

    let mut dst = me.output.to_writer()?;
    let mut dst = dst.lock();

    // FIXME: pick the correct album
    let album = manager.load_albums(catalog)?;
    let album = &album[0];
    match me.print_type {
        RepoPrintType::Title => writeln!(dst, "{}", album.title())?,
        RepoPrintType::Artist => writeln!(dst, "{}", album.artist())?,
        RepoPrintType::Date => writeln!(dst, "{}", album.release_date())?,
        RepoPrintType::Cue => {
            match album.discs().iter().nth(disc_id) {
                Some(disc) => {
                    write!(dst, r#"TITLE "{title}"
PERFORMER "{artist}"
REM DATE "{date}"
"#, title = disc.title(), artist = disc.artist(), date = album.release_date())?;
                    if me.add_generated_by {
                        write!(dst, r#"REM COMMENT "Generated by Anni v{}""#, crate_version!())?;
                    }

                    for (track_id, track) in disc.tracks().iter().enumerate() {
                        let track_id = track_id + 1;
                        write!(dst, r#"
FILE "{filename}" WAVE
  TRACK 01 AUDIO
    TITLE "{title}"
    PERFORMER "{artist}"
    INDEX 01 00:00:00"#,
                               filename = format!("{:02}. {}.flac", track_id, track.title().replace("/", "／")),
                               title = track.title(),
                               artist = track.artist(),
                        )?;
                    }
                }
                None => {
                    bail!("Disc {} not found!", disc_id + 1);
                }
            }
        }
        RepoPrintType::Toml => {
            write!(dst, "{}", album.to_string())?;
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
}

pub(crate) fn stream_to_track(stream: &FlacHeader) -> Track {
    match stream.comments() {
        Some(comment) => {
            let map = comment.to_map();
            Track::new(
                map.get("TITLE").map(|v| v.value()).unwrap_or("").to_string(),
                map.get("ARTIST").map(|v| v.value().to_string()),
                None,
                Default::default(),
            )
        }
        None => Track::empty()
    }
}

fn is_album_folder(input: &str) -> bool {
    let bytes = input.as_bytes();
    let second_last_byte = bytes[bytes.len() - 2];
    !(bytes[bytes.len() - 1] == b']' && second_last_byte > b'0' && second_last_byte < b'9')
}

////////////////////////////////////////////////////////////////////////
// Repo database
#[derive(Parser, Debug, Clone)]
pub struct RepoDatabaseAction {
    #[clap(about = ll ! {"export-to"})]
    output: PathBuf,
}

#[handler(RepoDatabaseAction)]
fn repo_database_action(me: RepoDatabaseAction, manager: RepositoryManager) -> anyhow::Result<()> {
    if !me.output.is_dir() {
        bail!("Output path must be a directory!");
    }

    let manager = manager.into_owned_manager()?;
    block_on(manager.to_database(&me.output.join("repo.db")))?;

    Ok(())
}

////////////////////////////////////////////////////////////////////////
// Repo migration
#[derive(Parser, Handler, Debug, Clone)]
pub struct RepoMigrateAction {
    #[clap(subcommand)]
    subcommand: RepoMigrateSubcommand,
}

#[derive(Parser, Handler, Debug, Clone)]
pub enum RepoMigrateSubcommand {
    #[clap(about = ll ! ("repo-migrate-album-id"))]
    #[clap(name = "album_id")]
    AlbumId(RepoMigrateAlbumIdAction),
}

#[derive(Parser, Debug, Clone)]
pub struct RepoMigrateAlbumIdAction;

#[handler(RepoMigrateAlbumIdAction)]
fn repo_migrate_album_id(repo: &RepoSubcommand) -> anyhow::Result<()> {
    let album_root = repo.root.join("album");

    use toml_edit::{Document, Item, Key, Table, value};
    for toml_path in fs::PathWalker::new(album_root, false)
        .filter(|p| p.is_file() && p.extension().unwrap_or_default() == "toml") {
        let mut doc = fs::read_to_string(&toml_path)
            .expect("Failed to read toml to string")
            .parse::<Document>()
            .expect("Invalid toml document");
        if !doc["album"].as_table().unwrap().contains_key("album_id") {
            let mut album = Table::new();
            album.set_position(0);
            album["album_id"] = value(uuid::Uuid::new_v4().to_string());
            for (k, v) in doc["album"].as_table().unwrap().clone().into_iter() {
                album.insert_formatted(&Key::new(k), v);
            }
            doc["album"] = Item::Table(album);
            // remove prefix \n, append \n
            let result = format!("{}\n", doc.to_string().trim());
            fs::write(toml_path, result).expect("Failed to write toml");
        }
    }
    Ok(())
}
