use anni_repo::album::{Track, Disc};
use anni_flac::Stream;
use clap::ArgMatches;
use anni_repo::structure::{album_info, disc_info, file_name};
use anni_repo::{Album, Repository};
use anni_utils::fs;
use crate::{flac, repo, Ret};
use std::path::{PathBuf, Path};
use shell_escape::escape;

struct RepoSettings {
    repo_root: PathBuf,
    album_root: PathBuf,
    repo: Repository,
}

impl RepoSettings {
    pub fn new(root: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let root = Path::new(root);
        let repo = root.join("repo.toml");
        Ok(Self {
            repo_root: root.to_owned(),
            album_root: root.join("album"),
            repo: Repository::from_file(repo),
        })
    }

    pub fn with_album(&self, catalog: &str) -> PathBuf {
        self.album_root.join(format!("{}.toml", catalog))
    }

    pub fn album_exists(&self, catalog: &str) -> bool {
        fs::metadata(self.with_album(catalog)).is_ok()
    }

    pub fn load_album(&self, catalog: &str) -> Album {
        Album::from_file(self.with_album(catalog))
    }
}

pub(crate) fn handle_repo(matches: &ArgMatches) -> Ret {
    let settings = RepoSettings::new(matches.value_of("repo.root").unwrap())?;

    if let Some(matches) = matches.subcommand_matches("apply") {
        handle_repo_apply(matches, &settings)?;
    } else if let Some(matches) = matches.subcommand_matches("add") {
        handle_repo_add(matches, &settings)?;
    } else {
        unimplemented!();
    }
    Ok(())
}

fn handle_repo_add(matches: &ArgMatches, settings: &RepoSettings) -> Ret {
    let to_add = Path::new(matches.value_of("Filename").unwrap());
    let last = anni_repo::structure::file_name(to_add)?;
    if last.ends_with("]") {
        return Err("You can only add a valid album directory in anni convention to anni metadata repository.".into());
    }

    let (release_date, catalog, title) = album_info(&last)?;
    if settings.album_exists(&catalog) {
        return Err("Album with the same catalog exists in repo. Aborted.".into());
    }

    let mut album = Album::new(&title, "Artist", release_date, &catalog);

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
            let (catalog, _, _) = disc_info(&*file_name(dir)?)?;
            Disc::new(&catalog)
        } else {
            Disc::new(&catalog)
        };
        for path in files.iter() {
            let stream = flac::parse_file(path.to_str().unwrap())?;
            let track = repo::stream_to_track(&stream);
            disc.add_track(track);
        }
        album.add_disc(disc);
    }

    fs::write(settings.with_album(&catalog), album.to_string())?;
    Ok(())
}

fn handle_repo_apply(matches: &ArgMatches, settings: &RepoSettings) -> Ret {
    let to_apply = Path::new(matches.value_of("Filename").unwrap());
    let last = anni_repo::structure::file_name(to_apply)?;
    if last.ends_with("]") {
        return Err("You can only apply album metadata to a valid anni convention album directory.".into());
    }

    let (release_date, catalog, album_title) = album_info(&last)?;
    if !settings.album_exists(&catalog) {
        return Err("Catalog not found in repo. Aborted.".into());
    }

    let album = settings.load_album(&catalog);
    if album.title() != album_title || album.catalog() != catalog || album.release_date() != &release_date {
        return Err("Album info mismatch. Aborted.".into());
    }

    let discs = album.discs();
    for (i, disc) in album.discs().iter().enumerate() {
        let disc_num = i + 1;
        let disc_dir = if discs.len() > 1 {
            to_apply.join(format!("[{}] {} [Disc {}]", disc.catalog(), album_title, disc_num))
        } else {
            to_apply.to_owned()
        };
        let files = fs::get_ext_files(disc_dir, "flac", false)?.unwrap();
        let tracks = disc.tracks();
        if files.len() != tracks.len() {
            return Err(format!("Track number mismatch in Disc {} of {}. Aborted.", disc_num, catalog).into());
        }

        for i in 0..files.len() {
            let file = &files[i];
            let track = &tracks[i];
            let meta = format!(r#"TITLE={}
ALBUM={}
ARTIST={}
DATE={}
TRACKNUMBER={}
TRACKTOTAL={}
DISCNUMBER={}
DISCTOTAL={}"#, track.title(), album_title, track.artist(), album.release_date(), i + 1, tracks.len(), disc_num, discs.len());
            println!("echo {} | metaflac --remove-all-tags --import-tags-from=- {}", escape(meta.into()), escape(file.to_str().unwrap().into()));
        }
    }
    Ok(())
}

pub(crate) fn stream_to_track(stream: &Stream) -> Track {
    let comment = stream.comments().unwrap();
    Track::new(comment["TITLE"].value(), Some(comment["ARTIST"].value()), None)
}
