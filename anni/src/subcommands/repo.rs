use anni_flac::FlacHeader;
use anni_repo::album::{Disc, Track};
use anni_repo::library::{album_info, disc_info, file_name};
use anni_repo::{Album, RepositoryManager};
use anni_common::fs;
use clap::{Clap, ArgEnum, crate_version};
use crate::{ll, ball};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use anni_flac::blocks::{UserComment, UserCommentExt};
use crate::cli::HandleArgs;

#[derive(Clap, Debug)]
#[clap(about = ll ! {"repo"})]
pub struct RepoSubcommand {
    #[clap(long, env = "ANNI_REPO")]
    #[clap(about = ll ! {"repo-root"})]
    root: PathBuf,

    #[clap(subcommand)]
    action: RepoAction,
}

impl HandleArgs for RepoSubcommand {
    fn handle(&self) -> anyhow::Result<()> {
        // TODO: read repo root from config
        let manager = RepositoryManager::new(self.root.as_path())?;
        self.action.handle(&manager)
    }
}

#[derive(Clap, Debug)]
pub enum RepoAction {
    #[clap(about = ll ! {"repo-add"})]
    Add(RepoAddAction),
    #[clap(about = ll ! {"repo-edit"})]
    Edit(RepoEditAction),
    #[clap(about = ll ! {"repo-apply"})]
    Apply(RepoApplyAction),
    #[clap(about = ll ! {"repo-validate"})]
    Validate(RepoValidateAction),
    #[clap(about = ll ! {"repo-print"})]
    Print(RepoPrintAction),
}

impl RepoAction {
    fn handle(&self, manager: &RepositoryManager) -> anyhow::Result<()> {
        match self {
            RepoAction::Add(add) => add.handle(manager),
            RepoAction::Edit(edit) => edit.handle(manager),
            RepoAction::Apply(apply) => apply.handle(manager),
            RepoAction::Validate(validate) => validate.handle(manager),
            RepoAction::Print(print) => print.handle(manager),
        }
    }
}

#[derive(Clap, Debug)]
pub struct RepoAddAction {
    #[clap(short = 'e', long)]
    #[clap(about = ll ! ("repo-add-edit"))]
    open_editor: bool,

    #[clap(required = true)]
    directories: Vec<PathBuf>,
}

impl RepoAddAction {
    fn handle(&self, manager: &RepositoryManager) -> anyhow::Result<()> {
        for to_add in self.directories.iter() {
            let last = anni_repo::library::file_name(&to_add)?;
            if !is_album_folder(&last) {
                ball!("repo-add-invalid-album");
            }

            let (release_date, catalog, album_title, discs) = album_info(&last)?;
            if manager.album_exists(&catalog) {
                ball!("repo-album-exists", catalog = catalog);
            }

            let mut album = Album::new(album_title.clone(), "[Unknown Artist]".to_string(), release_date, catalog.clone());

            let directories = fs::get_subdirectories(to_add)?;
            let mut directories: Vec<_> = directories.iter().map(|r| r.as_path()).collect();
            if discs == 1 {
                directories.push(&to_add);
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
                    )
                } else {
                    Disc::new(catalog.clone(), None, None, None)
                };
                for path in files.iter() {
                    let header = FlacHeader::from_file(path)?;
                    let track = stream_to_track(&header);
                    disc.push_track(track); // use push_track here to avoid metadata inherit
                }
                album.push_disc(disc); // the same
            }
            album.inherit();

            manager.add_album(&catalog, album)?;
            if self.open_editor {
                manager.edit_album(&catalog)?;
            }
        }
        Ok(())
    }
}

#[derive(Clap, Debug)]
pub struct RepoEditAction {
    #[clap(required = true)]
    directories: Vec<PathBuf>,
}

impl RepoEditAction {
    fn handle(&self, manager: &RepositoryManager) -> anyhow::Result<()> {
        // FIXME: handle all inputs
        let last = anni_repo::library::file_name(&self.directories[0])?;
        debug!(target: "repo|edit", "Directory: {}", last);
        if !is_album_folder(&last) {
            ball!("repo-add-invalid-album");
        }

        let (_, catalog, _, _) = album_info(&last)?;
        debug!(target: "repo|edit", "Catalog: {}", catalog);
        if !manager.album_exists(&catalog) {
            ball!("repo-album-not-found", catalog = catalog);
        }
        let file = manager.with_album(&catalog);
        edit::edit_file(&file)?;
        Ok(())
    }
}

#[derive(Clap, Debug)]
pub struct RepoApplyAction {
    #[clap(required = true)]
    directories: Vec<PathBuf>,
}

impl RepoApplyAction {
    fn handle(&self, manager: &RepositoryManager) -> anyhow::Result<()> {
        // FIXME: handle all inputs
        let to_apply = Path::new(&self.directories[0]);
        let last = anni_repo::library::file_name(to_apply)?;
        debug!(target: "repo|apply", "Directory: {}", last);
        if !is_album_folder(&last) {
            ball!("repo-add-invalid-album");
        }

        // extract album info
        let (release_date, catalog, album_title, disc_count) = album_info(&last)?;
        debug!(target: "repo|apply", "Release date: {}, Catalog: {}, Title: {}", release_date, catalog, album_title);
        if !manager.album_exists(&catalog) {
            ball!("repo-album-not-found", catalog = catalog);
        }

        // get track metadata & compare with album folder
        let album = manager.load_album(&catalog)?;
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
                to_apply.join(format!(
                    "[{catalog}] {title} [Disc {disc_num}]",
                    catalog = disc.catalog(),
                    title = disc.title(),
                    disc_num = disc_num,
                ))
            } else {
                to_apply.to_owned()
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
}

#[derive(Clap, Debug)]
pub struct RepoValidateAction;

impl RepoValidateAction {
    fn handle(&self, manager: &RepositoryManager) -> anyhow::Result<()> {
        info!(target: "anni", "Repository validation started.");
        for catalog in manager.catalogs()? {
            let album = manager.load_album(&catalog)?;
            if album.catalog() != catalog {
                error!(target: &format!("repo|{}", catalog), "Album catalog '{album_catalog}' does not match filename", album_catalog = album.catalog());
            }
            if album.artist() == "[Unknown Artist]" {
                error!(target: &format!("repo|{}", catalog), "Invalid artist '{artist}'", artist = album.artist());
            }
        }
        info!(target: "anni", "Repository validation finished.");
        Ok(())
    }
}

#[derive(Clap, Debug)]
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
}

impl RepoPrintAction {
    fn handle(&self, manager: &RepositoryManager) -> anyhow::Result<()> {
        let split: Vec<_> = self.catalog.split('/').collect();
        let (catalog, disc_id) = if split.len() == 1 {
            (split[0], 1)
        } else {
            (split[0], usize::from_str(split[1]).expect("Invalid disc id"))
        };
        let disc_id = if disc_id > 0 { disc_id - 1 } else { disc_id };

        if !manager.album_exists(catalog) {
            ball!("repo-album-not-found", catalog = catalog);
        }

        let album = manager.load_album(catalog)?;
        match self.print_type {
            RepoPrintType::Title => println!("{}", album.title()),
            RepoPrintType::Artist => println!("{}", album.artist()),
            RepoPrintType::Date => println!("{}", album.release_date()),
            RepoPrintType::Cue => {
                match album.discs().iter().nth(disc_id) {
                    Some(disc) => {
                        print!(r#"TITLE "{title}"
PERFORMER "{artist}"
REM DATE "{date}"
"#, title = disc.title(), artist = disc.artist(), date = album.release_date());
                        if self.add_generated_by {
                            print!(r#"REM COMMENT "Generated by Anni v{}""#, crate_version!());
                        }

                        for (track_id, track) in disc.tracks().iter().enumerate() {
                            let track_id = track_id + 1;
                            print!(r#"
FILE "{filename}" WAVE
  TRACK 01 AUDIO
    TITLE "{title}"
    PERFORMER "{artist}"
    INDEX 01 00:00:00"#,
                                   filename = format!("{:02}. {}.flac", track_id, track.title().replace("/", "／")),
                                   title = track.title(),
                                   artist = track.artist(),
                            );
                        }
                    }
                    None => {
                        bail!("Disc {} not found!", disc_id + 1);
                    }
                }
            }
            RepoPrintType::Toml => {
                print!("{}", album.to_string());
            }
        }
        Ok(())
    }
}

#[derive(ArgEnum, Debug, PartialEq)]
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
