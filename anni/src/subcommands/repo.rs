use anni_flac::FlacHeader;
use anni_repo::album::{Disc, Track};
use anni_repo::library::{album_info, disc_info, file_name};
use anni_repo::{Album, RepositoryManager};
use anni_utils::fs;
use clap::{ArgMatches, App, Arg, crate_version};
use crate::ball;
use shell_escape::escape;
use std::path::{Path, PathBuf};
use crate::subcommands::Subcommand;
use std::str::FromStr;
use crate::i18n::ClapI18n;

pub(crate) struct RepoSubcommand;

impl Subcommand for RepoSubcommand {
    fn name(&self) -> &'static str {
        "repo"
    }

    fn create(&self) -> App<'static> {
        App::new("repo")
            .about_ll("repo")
            .arg(Arg::new("repo.root")
                .about("repo-root")
                .long("root")
                .env("ANNI_REPO")
                .takes_value(true)
                .required(true)
            )
            .subcommand(App::new("add")
                .about_ll("repo-add")
                .arg(Arg::new("edit")
                    .about_ll("repo-add-edit")
                    .long("edit")
                    .short('e')
                )
                .arg(Arg::new("Directories")
                    .takes_value(true)
                    .required(true)
                    .min_values(1)
                )
            )
            .subcommand(App::new("edit")
                .about_ll("repo-edit")
                .arg(Arg::new("Directory")
                    .takes_value(true)
                    .required(true)
                )
            )
            .subcommand(App::new("apply")
                .about_ll("repo-apply")
                .arg(Arg::new("Directory")
                    .takes_value(true)
                    .required(true)
                )
            )
            .subcommand(App::new("print")
                .about_ll("repo-print")
                .arg(Arg::new("type")
                    .about("repo-print-type")
                    .long("type")
                    .short('t')
                    .takes_value(true)
                    .required(true)
                    .possible_values(&["title", "artist", "date", "cue", "toml"])
                    .default_value("title")
                )
                .arg(Arg::new("clean")
                    .about_ll("repo-print-clean")
                    .long("clean")
                    .short('c')
                )
                .replace("--title", &["--type=title"])
                .replace("--artist", &["--type=artist"])
                .replace("--date", &["--type=date"])
                .replace("--cue", &["--type=cue"])
                .replace("--toml", &["--type=toml"])
                .arg(Arg::new("Catalog")
                    .about_ll("repo-print-catalog")
                    .takes_value(true)
                    .required(true)
                )
            )
    }

    fn handle(&self, matches: &ArgMatches) -> anyhow::Result<()> {
        // TODO: read repo root from config
        let settings = RepositoryManager::new(matches.value_of("repo.root").unwrap())?;

        let (subcommand, matches) = matches.subcommand().unwrap();
        debug!("Repo subcommand matched: {}", subcommand);
        match subcommand {
            "apply" => handle_repo_apply(matches, &settings)?,
            "edit" => handle_repo_edit(matches, &settings)?,
            "add" => handle_repo_add(matches, &settings)?,
            "print" => handle_repo_print(matches, &settings)?,
            _ => unimplemented!()
        }
        Ok(())
    }
}

fn handle_repo_add(matches: &ArgMatches, settings: &RepositoryManager) -> anyhow::Result<()> {
    let to_add = matches.values_of_os("Directories").unwrap();
    for to_add in to_add {
        let to_add = Path::new(to_add);
        let last = anni_repo::library::file_name(to_add)?;
        if !is_album_folder(&last) {
            ball!("repo-add-invalid-album");
        }

        let (release_date, catalog, album_title, discs) = album_info(&last)?;
        if settings.album_exists(&catalog) {
            ball!("repo-album-exists", catalog = catalog);
        }

        let mut album = Album::new(album_title.clone(), "[Unknown Artist]".to_string(), release_date, catalog.clone());

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
                )
            } else {
                Disc::new(catalog.clone(), None, None, None)
            };
            for path in files.iter() {
                let header = FlacHeader::from_file(path)?;
                let track = stream_to_track(&header);
                disc.add_track(track);
            }
            album.add_disc(disc);
        }

        album.format();
        settings.add_album(&catalog, album)?;
        if matches.is_present("edit") {
            settings.edit_album(&catalog)?;
        }
    }
    Ok(())
}

fn handle_repo_edit(matches: &ArgMatches, settings: &RepositoryManager) -> anyhow::Result<()> {
    let to_add = Path::new(matches.value_of_os("Directory").unwrap());
    let last = anni_repo::library::file_name(to_add)?;
    debug!("Edit directory: {}", last);
    if !is_album_folder(&last) {
        ball!("repo-add-invalid-album");
    }

    let (_, catalog, _, _) = album_info(&last)?;
    debug!("Catalog: {}", catalog);
    if !settings.album_exists(&catalog) {
        ball!("repo-album-not-found", catalog = catalog);
    }
    let file = settings.with_album(&catalog);
    edit::edit_file(&file)?;
    Ok(())
}

fn handle_repo_apply(matches: &ArgMatches, settings: &RepositoryManager) -> anyhow::Result<()> {
    let to_apply = Path::new(matches.value_of_os("Directory").unwrap());
    let last = anni_repo::library::file_name(to_apply)?;
    debug!("Apply directory: {}", last);
    if !is_album_folder(&last) {
        ball!("repo-add-invalid-album");
    }

    // extract album info
    let (release_date, catalog, album_title, disc_count) = album_info(&last)?;
    debug!("Release date: {}, Catalog: {}, Title: {}", release_date, catalog, album_title);
    if !settings.album_exists(&catalog) {
        ball!("repo-album-not-found", catalog = catalog);
    }

    // get track metadata & compare with album folder
    let album = settings.load_album(&catalog)?;
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

    let mut output = Vec::new();
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
        debug!("Disc dir: {:?}", disc_dir);

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

            let flac = FlacHeader::from_file(file)?;
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
                track_total = tracks.len(),
                disc_number = disc_num,
                disc_total = discs.len(),
            );
            // no comment block exist, or comments is not correct
            if comments.is_none() || comments.unwrap().to_string() != meta {
                output.push(format!(
                    "echo {} | metaflac --remove-all-tags --import-tags-from=- {}",
                    escape(meta.trim().into()),
                    escape(file.to_str().unwrap().into())
                ));
            }
        }
    }

    for meta in output {
        println!("{}", meta);
    }
    Ok(())
}

pub(crate) fn stream_to_track(stream: &FlacHeader) -> Track {
    if let Some(comment) = stream.comments() {
        let map = comment.to_map();
        Track::new(
            map.get("TITLE").map(|v| v.value()).unwrap_or("").to_string(),
            map.get("ARTIST").map(|v| v.value()),
            None,
        )
    } else {
        Track::new(String::new(), None, None)
    }
}

fn is_album_folder(input: &str) -> bool {
    let bytes = input.as_bytes();
    let second_last_byte = bytes[bytes.len() - 2];
    !(bytes[bytes.len() - 1] == b']' && second_last_byte > b'0' && second_last_byte < b'9')
}

fn handle_repo_print(matches: &ArgMatches, settings: &RepositoryManager) -> anyhow::Result<()> {
    let catalog = matches.value_of("Catalog").unwrap();
    let split: Vec<_> = catalog.split('/').collect();
    let (catalog, disc_id) = if split.len() == 1 {
        (split[0], 1)
    } else {
        (split[0], usize::from_str(split[1]).expect("Invalid disc id"))
    };
    let disc_id = if disc_id > 0 { disc_id - 1 } else { disc_id };

    if !settings.album_exists(catalog) {
        ball!("repo-album-not-found", catalog = catalog);
    }

    let album = settings.load_album(catalog)?;
    match matches.value_of("type").unwrap() {
        "title" => println!("{}", album.title()),
        "artist" => println!("{}", album.artist()),
        "date" => println!("{}", album.release_date()),
        "cue" => {
            match album.discs().iter().nth(disc_id) {
                Some(disc) => {
                    print!(r#"TITLE "{title}"
PERFORMER "{artist}"
REM DATE "{date}"
"#, title = disc.title(), artist = disc.artist(), date = album.release_date());
                    if !matches.is_present("clean") {
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
        "toml" => {
            let mut album = album;
            album.format();
            print!("{}", album.to_string());
        }
        _ => unimplemented!()
    }
    Ok(())
}
