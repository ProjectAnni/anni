use anni_flac::FlacHeader;
use anni_repo::album::{Disc, Track};
use anni_repo::library::{album_info, disc_info, file_name};
use anni_repo::{Album, RepositoryManager};
use anni_utils::fs;
use clap::{ArgMatches, App, Arg};
use crate::{fl, ball};
use shell_escape::escape;
use std::path::{Path, PathBuf};
use crate::subcommands::Subcommand;

pub(crate) struct RepoSubcommand;

impl Subcommand for RepoSubcommand {
    fn name(&self) -> &'static str {
        "repo"
    }

    fn create(&self) -> App<'static> {
        App::new("repo")
            .about(fl!("repo"))
            .arg(Arg::new("repo.root")
                .about(fl!("repo-root"))
                .long("root")
                .env("ANNI_ROOT")
                .takes_value(true)
                .required(true)
            )
            .subcommand(App::new("add")
                .about(fl!("repo-add"))
                .arg(Arg::new("edit")
                    .about(fl!("repo-add-edit"))
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
                .about(fl!("repo-edit"))
                .arg(Arg::new("Directory")
                    .takes_value(true)
                    .required(true)
                )
            )
            .subcommand(App::new("apply")
                .about(fl!("repo-apply"))
                .arg(Arg::new("Directory")
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

        let (release_date, catalog, album_title) = album_info(&last)?;
        if settings.album_exists(&catalog) {
            ball!("repo-album-exists", catalog = catalog);
        }

        let mut album = Album::new(&album_title, "[Unknown Artist]", release_date, &catalog);

        let directories = fs::get_subdirectories(to_add)?;
        let mut directories: Vec<_> = directories.iter().map(|r| r.as_path()).collect();
        let mut has_discs = true;
        if directories.len() == 0 {
            directories.push(to_add);
            has_discs = false;
        }

        for dir in directories.iter() {
            let files = fs::get_ext_files(PathBuf::from(dir), "flac", false)?.unwrap();
            let mut disc = if has_discs {
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

    let (_, catalog, _) = album_info(&last)?;
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
    let (release_date, catalog, album_title) = album_info(&last)?;
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
    for (disc_num, disc) in album.discs().iter().enumerate() {
        let disc_num = disc_num + 1;
        let title = disc.title().unwrap_or(album_title.as_str());
        let disc_dir = if discs.len() > 1 {
            to_apply.join(format!(
                "[{catalog}] {title} [Disc {disc_num}]",
                catalog = disc.catalog(),
                title = title,
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
            let meta = format!(
                r#"TITLE={title}
ALBUM={album}
ARTIST={artist}
DATE={release_date}
TRACKNUMBER={track_number}
TRACKTOTAL={track_total}
DISCNUMBER={disc_number}
DISCTOTAL={disc_total}"#,
                title = track.title(),
                album = album_title,
                artist = track.artist(),
                release_date = album.release_date(),
                track_number = track_num,
                track_total = tracks.len(),
                disc_number = disc_num,
                disc_total = discs.len(),
            );
            println!(
                "echo {} | metaflac --remove-all-tags --import-tags-from=- {}",
                escape(meta.into()),
                escape(file.to_str().unwrap().into())
            );
        }
    }
    Ok(())
}

pub(crate) fn stream_to_track(stream: &FlacHeader) -> Track {
    if let Some(comment) = stream.comments() {
        let map = comment.to_map();
        Track::new(
            map.get("TITLE").map(|v| v.value()).unwrap_or(""),
            map.get("ARTIST").map(|v| v.value()),
            None,
        )
    } else {
        Track::new("", None, None)
    }
}

fn is_album_folder(input: &str) -> bool {
    let bytes = input.as_bytes();
    let second_last_byte = bytes[bytes.len() - 2];
    !(bytes[bytes.len() - 1] == b']' && second_last_byte > b'0' && second_last_byte < b'9')
}
