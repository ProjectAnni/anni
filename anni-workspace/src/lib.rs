pub mod config;
mod error;
mod state;
mod utils;

use crate::config::WorkspaceConfig;
use anni_common::fs;
use anni_repo::library::file_name;
use anni_repo::prelude::{AnniDate, UNKNOWN_ARTIST};
use anni_repo::RepositoryManager;
use config::LibraryConfig;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::num::NonZeroU8;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use utils::lock::WorkspaceAlbumLock;
use uuid::Uuid;

pub use error::WorkspaceError;
pub use state::*;

const IGNORED_LIST: [&str; 2] = [
    ".directory", // KDE Dolphin
    ".DS_Store",  // macOS
];

pub struct AnniWorkspace {
    /// Full path of `.anni` directory.
    dot_anni: PathBuf,
}

impl AnniWorkspace {
    /// # Safety
    ///
    /// If you're sure that the directory is `.anni`, you can use this method to avoid unnecessary checking steps.
    pub unsafe fn new_unchecked(dot_anni: PathBuf) -> Self {
        AnniWorkspace { dot_anni }
    }

    /// Find [AnniWorkspace] from current working directory.
    pub fn new() -> Result<Self, WorkspaceError> {
        Self::find(std::env::current_dir()?)
    }

    /// Open a [AnniWorkspace] from given path.
    ///
    /// If the path is not a valid workspace, [WorkspaceError::NotAWorkspace] will be returned.
    pub fn open<P>(path: P) -> Result<Self, WorkspaceError>
    where
        P: AsRef<Path>,
    {
        let dot_anni = path.as_ref().join(".anni");
        if dot_anni.exists() {
            let config_path = dot_anni.join("config.toml");
            if config_path.exists() {
                return Ok(Self { dot_anni });
            }
        }

        Err(WorkspaceError::NotAWorkspace)
    }

    /// Find and open a [AnniWorkspace] from given path.
    ///
    /// This method will try to open all parent directories until it finds a valid workspace.
    /// If workspace is not found, [WorkspaceError::WorkspaceNotFound] will be returned.
    pub fn find<P>(path: P) -> Result<Self, WorkspaceError>
    where
        P: AsRef<Path>,
    {
        let mut path = path.as_ref();
        loop {
            let workspace = Self::open(path);
            if workspace.is_ok() {
                return workspace;
            }
            path = path.parent().ok_or(WorkspaceError::WorkspaceNotFound)?;
        }
    }

    /// Get root path of the workspace
    pub fn workspace_root(&self) -> &Path {
        self.dot_anni.parent().unwrap()
    }

    /// Get root path of the metadata repository.
    ///
    /// # Warn
    /// This method may be removed in the future
    pub fn repo_root(&self) -> PathBuf {
        self.dot_anni.join("repo")
    }

    /// Get root path of internal audio files
    pub fn objects_root(&self) -> PathBuf {
        self.dot_anni.join("objects")
    }

    /// Get album id from symlink target.
    ///
    /// Returns [WorkspaceError::NotAnAlbum] if the symlink is not valid.
    pub fn get_album_id<P>(&self, path: P) -> Result<Uuid, WorkspaceError>
    where
        P: AsRef<Path>,
    {
        let album_path = path.as_ref().join(".album");

        // 1. validate album path
        // if it does not exist, or is not a symlink, return None
        if !album_path.is_symlink() {
            return Err(WorkspaceError::NotAnAlbum(path.as_ref().to_path_buf()));
        }

        // 2. get album_id
        let real_path = fs::read_link(album_path)?;
        let album_id = real_path.file_name().unwrap().to_string_lossy();
        let album_id = Uuid::parse_str(&album_id)?;
        Ok(album_id)
    }

    /// Get controlled path of an album with album id.
    pub fn get_album_controlled_path(&self, album_id: &Uuid) -> Result<PathBuf, WorkspaceError> {
        let path = self.controlled_album_path(album_id, 2);
        if !path.exists() {
            return Err(WorkspaceError::AlbumNotFound(*album_id));
        }

        Ok(path)
    }

    /// Get album path with given `album_id` in workspace with no extra checks.
    pub fn controlled_album_path(&self, album_id: &Uuid, layer: usize) -> PathBuf {
        AnniWorkspace::strict_album_path(self.objects_root(), album_id, layer)
    }

    pub fn strict_album_path(mut root: PathBuf, album_id: &Uuid, layer: usize) -> PathBuf {
        let bytes = album_id.as_bytes();

        for byte in &bytes[0..layer] {
            root.push(format!("{byte:x}"));
        }
        root.push(album_id.to_string());

        root
    }

    /// Try to get [WorkspaceAlbum] from given path
    pub fn get_workspace_album<P>(&self, path: P) -> Result<WorkspaceAlbum, WorkspaceError>
    where
        P: AsRef<Path>,
    {
        let album_id = self.get_album_id(path.as_ref())?;
        let path = path.as_ref().to_path_buf();

        // valid album_id, it's an album directory
        let album_controlled_path = self.get_album_controlled_path(&album_id);
        Ok(WorkspaceAlbum {
            album_id,
            state: match album_controlled_path {
                Ok(controlled_path) => {
                    if !path.join(".album").exists() {
                        // symlink is broken
                        WorkspaceAlbumState::Dangling(path)
                    } else if fs::read_dir(controlled_path)?.next().is_some() {
                        // controlled part is not empty
                        WorkspaceAlbumState::Committed(path)
                    } else {
                        // controlled part is empty
                        WorkspaceAlbumState::Untracked(path)
                    }
                }
                // controlled part does not exist
                Err(WorkspaceError::AlbumNotFound(_)) => WorkspaceAlbumState::Dangling(path),
                _ => unreachable!(),
            },
        })
    }

    /// Scan the whole workspace and return all available albums
    pub fn scan(&self) -> Result<Vec<WorkspaceAlbum>, WorkspaceError> {
        let mut albums = BTreeMap::new();
        self.scan_userland_directory(&mut albums, self.workspace_root())?;
        self.scan_controlled_directory(&mut albums, self.objects_root(), 2)?;
        Ok(albums.into_values().collect())
    }

    /// Internal: scan userland
    fn scan_userland_directory<P>(
        &self,
        albums: &mut BTreeMap<Uuid, WorkspaceAlbum>,
        path: P,
    ) -> Result<(), WorkspaceError>
    where
        P: AsRef<Path>,
    {
        for entry in fs::read_dir(path.as_ref())? {
            let entry = entry?;
            if entry.file_name() == ".anni" {
                continue;
            }

            let metadata = entry.metadata()?;
            if metadata.is_dir() {
                // look for .album folder
                match self.get_workspace_album(entry.path()) {
                    // valid album_id, it's an album directory
                    Ok(album) => {
                        albums.insert(album.album_id.clone(), album);
                    }
                    // symlink was not found, scan recursively
                    Err(WorkspaceError::NotAnAlbum(path)) => {
                        self.scan_userland_directory(albums, path)?
                    }
                    Err(e) => return Err(e),
                }
            }
        }

        Ok(())
    }

    /// Internal: scan controlled part
    fn scan_controlled_directory<P>(
        &self,
        albums: &mut BTreeMap<Uuid, WorkspaceAlbum>,
        parent: P,
        level: u8,
    ) -> Result<(), WorkspaceError>
    where
        P: AsRef<Path>,
    {
        let parent = parent.as_ref();
        for entry in fs::read_dir(parent)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                if level > 0 {
                    self.scan_controlled_directory(albums, path, level - 1)?;
                } else {
                    let album_id = file_name(&path)?;
                    let album_id = Uuid::from_str(&album_id)?;
                    let is_published = path.join(".publish").exists();
                    albums.entry(album_id).or_insert_with(|| WorkspaceAlbum {
                        album_id,
                        state: if is_published {
                            WorkspaceAlbumState::Published
                        } else {
                            WorkspaceAlbumState::Garbage
                        },
                    });
                }
            }
        }
        Ok(())
    }

    /// Create album with given `album_id` and `discs` at given `path`
    ///
    /// Creation would fail if:
    /// - Album with given `album_id` already exists in workspace
    /// - Directory at `path` is an album directory
    pub fn create_album<P>(
        &self,
        album_id: &Uuid,
        userland_path: P,
        discs: NonZeroU8,
    ) -> Result<(), WorkspaceError>
    where
        P: AsRef<Path>,
    {
        let controlled_path = self.controlled_album_path(album_id, 2);
        if controlled_path.exists() {
            return Err(WorkspaceError::DuplicatedAlbumId(*album_id));
        }

        if let Ok(album_id) = self.get_album_id(userland_path.as_ref()) {
            // `path` is an album directory
            return Err(WorkspaceError::AlbumExists {
                album_id,
                path: userland_path.as_ref().to_path_buf(),
            });
        }

        // create album directories and symlink
        fs::create_dir_all(&controlled_path)?;
        fs::create_dir_all(&userland_path)?;
        fs::symlink_dir(&controlled_path, userland_path.as_ref().join(".album"))?;

        // create disc directories
        let discs = discs.get();
        if discs > 1 {
            for i in 1..=discs {
                let disc_path = userland_path.as_ref().join(format!("Disc {i}"));
                fs::create_dir_all(&disc_path)?;
            }
        }

        Ok(())
    }

    pub fn to_repository_manager(&self) -> Result<RepositoryManager, WorkspaceError> {
        Ok(RepositoryManager::new(self.repo_root())?)
    }

    pub fn get_config(&self) -> Result<WorkspaceConfig, WorkspaceError> {
        WorkspaceConfig::new(&self.dot_anni)
    }
}

pub struct ExtractedAlbumInfo<'a> {
    pub release_date: AnniDate,
    pub catalog: Cow<'a, str>,
    pub title: Cow<'a, str>,
    pub edition: Option<Cow<'a, str>>,
}

// Operations
impl AnniWorkspace {
    /// Get album or disc cover path from album or disc path
    ///
    /// `path` MUST be a valid album or disc path
    fn album_disc_cover_path<P>(path: P) -> PathBuf
    where
        P: AsRef<Path>,
    {
        path.as_ref().join("cover.jpg")
    }

    /// Take a overview of an untracked album directory.
    ///
    /// If the path provided is not an UNTRACKED album directory, or the album is incomplete, an error will be returned.
    pub fn get_untracked_album_overview<P>(
        &self,
        album_path: P,
    ) -> Result<UntrackedWorkspaceAlbum, WorkspaceError>
    where
        P: AsRef<Path>,
    {
        let album = self.get_workspace_album(album_path.as_ref())?;
        let album_path = match album.state {
            // check current state of the album
            WorkspaceAlbumState::Untracked(p) => p,
            state => {
                return Err(WorkspaceError::InvalidAlbumState(state));
            }
        };

        // validate album cover
        let album_cover = AnniWorkspace::album_disc_cover_path(&album_path);
        if !album_cover.exists() {
            return Err(WorkspaceError::CoverNotFound(album_cover));
        }

        // iterate over me.path to find all discs
        let flac_in_album_root = fs::get_ext_file(&album_path, "flac", false)?.is_some();
        let mut discs = fs::get_subdirectories(&album_path)?;

        // if there's only one disc, then there should be no sub directories, [true, true]
        // if there are multiple discs, then there should be no flac files in the root directory, [false, false]
        // other conditions are invalid
        if flac_in_album_root ^ discs.is_empty() {
            // both files and discs are empty, or both are not empty
            return Err(WorkspaceError::InvalidAlbumDiscStructure(
                album_path.clone(),
            ));
        }

        // add album as disc if there's only one disc
        if flac_in_album_root {
            discs.push(album_path.clone());
        }

        alphanumeric_sort::sort_path_slice(&mut discs);
        let discs = discs
            .into_iter()
            .enumerate()
            .map(|(index, disc_path)| {
                let index = index + 1;

                // iterate over all flac files
                let mut files = fs::read_dir(&disc_path)?
                    .filter_map(|e| {
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
                    })
                    .collect::<Vec<_>>();
                alphanumeric_sort::sort_path_slice(&mut files);

                let disc_cover = AnniWorkspace::album_disc_cover_path(&disc_path);
                if !disc_cover.exists() {
                    return Err(WorkspaceError::CoverNotFound(disc_cover));
                }

                Ok(UntrackedWorkspaceDisc {
                    index,
                    path: disc_path,
                    cover: disc_cover,
                    tracks: files,
                })
            })
            .collect::<Result<Vec<_>, WorkspaceError>>()?;

        Ok(UntrackedWorkspaceAlbum {
            album_id: album.album_id,
            path: album_path,
            simplified: flac_in_album_root,
            discs,
        })
    }

    /// Add album to workspace
    ///
    /// `Untracked` -> `Committed`
    pub fn commit<P, V>(&self, path: P, validator: Option<V>) -> Result<Uuid, WorkspaceError>
    where
        P: AsRef<Path>,
        V: FnOnce(&UntrackedWorkspaceAlbum) -> bool,
    {
        let album = self.get_untracked_album_overview(path)?;

        // validate album lock
        let lock = WorkspaceAlbumLock::new(&album.path)?;

        if let Some(validator) = validator {
            let pass = validator(&album);
            if !pass {
                return Err(WorkspaceError::UserAborted);
            }
        }

        let album_id = album.album_id;
        let album_path = album.path;

        // Add action
        // 1. lock album
        lock.lock()?;

        // 2. copy or move album cover
        let album_cover = AnniWorkspace::album_disc_cover_path(&album_path);
        let album_controlled_path = self.get_album_controlled_path(&album_id)?;
        let album_cover_controlled = AnniWorkspace::album_disc_cover_path(&album_controlled_path);
        if album.simplified {
            // cover might be used by discs, copy it
            fs::copy(&album_cover, &album_cover_controlled)?;
        } else {
            // move directly
            fs::rename(&album_cover, &album_cover_controlled)?;
            fs::symlink_file(&album_cover_controlled, &album_cover)?;
        }

        // 3. move discs
        for disc in album.discs.iter() {
            let disc_controlled_path = album_controlled_path.join(disc.index.to_string());
            fs::create_dir_all(&disc_controlled_path)?;

            // move tracks
            for (index, track_path) in disc.tracks.iter().enumerate() {
                let index = index + 1;
                let track_controlled_path = disc_controlled_path.join(format!("{index}.flac"));
                fs::rename(track_path, &track_controlled_path)?;
                fs::symlink_file(&track_controlled_path, track_path)?;
            }

            // move disc cover
            let disc_cover_controlled_path =
                AnniWorkspace::album_disc_cover_path(&disc_controlled_path);
            fs::rename(&disc.cover, &disc_cover_controlled_path)?;
            fs::symlink_file(&disc_cover_controlled_path, &disc.cover)?;
        }

        Ok(album_id)
    }

    /// Import tag from **committed** album.
    pub fn import_tags<P, E>(
        &self,
        album_path: P,
        extractor: E,
        allow_duplicate: bool,
    ) -> Result<Uuid, WorkspaceError>
    where
        P: AsRef<Path>,
        E: FnOnce(&str) -> Option<ExtractedAlbumInfo>,
    {
        use anni_repo::prelude::{Album, AlbumInfo, Disc, DiscInfo};

        let album_id = self.get_album_id(album_path.as_ref())?;
        let repo = self.to_repository_manager()?;
        let folder_name = file_name(&album_path)?;
        let ExtractedAlbumInfo {
            release_date,
            catalog,
            title,
            edition,
            ..
        } = extractor(&folder_name).ok_or_else(|| WorkspaceError::FailedToExtractAlbumInfo)?;

        let album_path = self.get_album_controlled_path(&album_id)?;
        let mut discs = Vec::new();
        loop {
            let disc_id = discs.len() + 1;
            let disc_path = album_path.join(disc_id.to_string());
            if !disc_path.exists() {
                break;
            }

            let mut tracks = Vec::new();
            loop {
                let track_id = tracks.len() + 1;
                let track_path = disc_path.join(format!("{track_id}.flac"));
                if !track_path.exists() {
                    break;
                }

                let flac = anni_flac::FlacHeader::from_file(&track_path).map_err(|error| {
                    WorkspaceError::FlacError {
                        path: track_path,
                        error,
                    }
                })?;
                tracks.push(flac.into())
            }
            discs.push(Disc::new(
                DiscInfo::new(
                    catalog.to_string(),
                    None,
                    None,
                    None,
                    None,
                    Default::default(),
                ),
                tracks,
            ));
        }

        let album = Album::new(
            AlbumInfo {
                album_id,
                title: title.to_string(),
                edition: edition.map(|c| c.to_string()),
                artist: UNKNOWN_ARTIST.to_string(),
                release_date,
                catalog: catalog.to_string(),
                ..Default::default()
            },
            discs,
        );
        repo.add_album(album, allow_duplicate)?;

        Ok(album_id)
    }

    pub fn revert<P>(&self, path: P) -> Result<(), WorkspaceError>
    where
        P: AsRef<Path>,
    {
        let album = self.get_workspace_album(path)?;
        match album.state {
            WorkspaceAlbumState::Committed(album_path) => {
                let lock = WorkspaceAlbumLock::new(&album_path)?;
                lock.lock()?;

                let album_controlled_path = self.get_album_controlled_path(&album.album_id)?;

                // recover files from controlled album path
                AnniWorkspace::recover_symlinks(&album_path)?;

                // remove and re-create controlled album path
                fs::remove_dir_all(&album_controlled_path, true)?;
                fs::create_dir_all(&album_controlled_path)?;

                Ok(())
            }
            state => Err(WorkspaceError::InvalidAlbumState(state)),
        }
    }

    fn recover_symlinks<P: AsRef<Path>>(path: P) -> Result<(), WorkspaceError> {
        log::debug!("Recovering path: {}", path.as_ref().display());
        let metadata = fs::symlink_metadata(path.as_ref())?;
        if metadata.is_symlink() {
            // ignore .album directories
            if let Some(file_name) = path.as_ref().file_name() {
                if file_name == ".album" {
                    return Ok(());
                }
            }

            // copy pointing file to current path
            let actual_path = fs::canonicalize(path.as_ref())?;
            log::debug!("Actual path: {}", actual_path.display());
            fs::rename(actual_path, path)?;
        } else if metadata.is_dir() {
            for entry in path.as_ref().read_dir()? {
                let entry = entry?;
                AnniWorkspace::recover_symlinks(entry.path())?;
            }
        }

        Ok(())
    }

    pub fn apply_tags<P>(&self, album_path: P) -> Result<(), WorkspaceError>
    where
        P: AsRef<Path>,
    {
        let album_id = self.get_album_id(album_path)?;
        let controlled_album_path = self.get_album_controlled_path(&album_id)?;

        let repo = self.to_repository_manager()?;
        let repo = repo.into_owned_manager()?;

        // TODO: do not panic here
        let album = repo
            .album(&album_id)
            .expect("Album not found in metadata repository");
        album.apply_strict(controlled_album_path)?;

        Ok(())
    }

    pub fn publish<P>(&self, album_path: P, soft: bool) -> Result<(), WorkspaceError>
    where
        P: AsRef<Path>,
    {
        let config = self.get_config()?;

        let publish_to = config
            .publish_to()
            .expect("Target audio library is not specified in workspace config file.");

        // valdiate target path
        if !publish_to.path.exists() {
            return Err(WorkspaceError::PublishTargetNotFound(
                publish_to.path.clone(),
            ));
        }

        let album = self.get_workspace_album(album_path)?;
        match album.state {
            WorkspaceAlbumState::Committed(album_path) => {
                // validate current path first
                // if normal files exist, abort the operation
                for file in fs::PathWalker::new(&album_path, true, false, Default::default()) {
                    let file_name = file
                        .file_name()
                        .and_then(|r| r.to_str())
                        .unwrap_or_default();
                    if IGNORED_LIST.contains(&file_name) {
                        // skip ignored files
                        continue;
                    }

                    return Err(WorkspaceError::UnexpectedFile(file));
                }

                // TODO: validate whether track number matches in the repository
                if let Some(layers) = publish_to.layers {
                    // publish as strict
                    self.do_publish_strict(album_path, publish_to, layers, soft)?;
                } else {
                    // publish as convention
                    unimplemented!()
                }

                Ok(())
            }
            state => Err(WorkspaceError::InvalidAlbumState(state)),
        }
    }

    fn do_publish_strict<P>(
        &self,
        album_path: P,
        publish_to: &LibraryConfig,
        layers: usize,
        soft: bool,
    ) -> Result<(), WorkspaceError>
    where
        P: AsRef<Path>,
    {
        let album_id = self.get_album_id(album_path.as_ref())?;
        let album_controlled_path = self.get_album_controlled_path(&album_id)?;

        // publish as strict
        // 1. get destination path
        let result_path =
            AnniWorkspace::strict_album_path(publish_to.path.clone(), &album_id, layers);
        let result_parent = result_path.parent().expect("Invalid path");

        // 2. create parent directory
        if !result_parent.exists() {
            fs::create_dir_all(&result_parent)?;
        }

        // 3. move/copy album
        if soft {
            // copy the whole album
            fs::copy_dir(&album_controlled_path, &result_path)?;
            // add soft published mark
            fs::write(album_controlled_path.join(".publish"), "")?;
        } else {
            // move directory
            fs::move_dir(&album_controlled_path, &result_path)?;
        }
        // 4. clean album folder
        fs::remove_dir_all(&album_path, true)?; // TODO: add an option to disable trash feature

        Ok(())
    }
}
