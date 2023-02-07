pub mod config;
mod error;
mod state;

use crate::config::WorkspaceConfig;
use anni_common::fs;
use anni_repo::library::file_name;
use anni_repo::prelude::AnniDate;
use anni_repo::RepositoryManager;
pub use error::WorkspaceError;
pub use state::*;
use std::borrow::Cow;
use std::collections::HashMap;
use std::num::NonZeroU8;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use uuid::Uuid;

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

    pub fn find<P>(path: P) -> Result<Self, WorkspaceError>
    where
        P: AsRef<Path>,
    {
        let mut path = path.as_ref();
        loop {
            let dot_anni = path.join(".anni");
            if dot_anni.exists() {
                let config_path = dot_anni.join("config.toml");
                return if config_path.exists() {
                    Ok(Self { dot_anni })
                } else {
                    Err(WorkspaceError::ConfigNotFound(config_path))
                };
            }
            path = path.parent().ok_or(WorkspaceError::WorkspaceNotFound)?;
        }
    }

    /// Get root path of the workspace
    pub fn workspace_root(&self) -> &Path {
        self.dot_anni.parent().unwrap()
    }

    /// Get root path of the metadata repository
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

    /// Get album id from symlink target
    ///
    /// Returns `WorkspaceError::NotAnAlbum` if the symlink is not valid
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

    /// Get controlled path of an album with album id
    pub fn get_album_controlled_path(&self, album_id: &Uuid) -> Result<PathBuf, WorkspaceError> {
        let path = self.strict_album_path(album_id, 2);
        if !path.exists() {
            return Err(WorkspaceError::AlbumNotFound(*album_id));
        }

        Ok(path)
    }

    /// Get album path with given `album_id` in workspace with no extra checks
    pub fn strict_album_path(&self, album_id: &Uuid, layer: usize) -> PathBuf {
        let mut res = self.objects_root();
        let bytes = album_id.as_bytes();

        for i in 0..layer {
            let byte = bytes[i];
            res.push(format!("{byte:x}"));
        }
        res.push(album_id.to_string());

        res
    }

    /// Try to get `WorkspaceAlbum` from given path
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
        let mut albums = HashMap::new();
        self.scan_userland_directory(&mut albums, self.workspace_root())?;
        self.scan_controlled_directory(&mut albums, self.objects_root(), 2)?;
        Ok(albums.into_values().collect())
    }

    /// Internal: scan userland
    fn scan_userland_directory<P>(
        &self,
        albums: &mut HashMap<Uuid, WorkspaceAlbum>,
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
        albums: &mut HashMap<Uuid, WorkspaceAlbum>,
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
        let controlled_path = self.strict_album_path(album_id, 2);
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

// add discs
pub struct WorkspaceDisc {
    pub index: usize,
    pub path: PathBuf,
    pub cover: PathBuf,
    pub tracks: Vec<PathBuf>,
}

pub struct ExtractedAlbumInfo<'a> {
    pub release_date: AnniDate,
    pub catalog: Cow<'a, str>,
    pub title: Cow<'a, str>,
    pub edition: Option<Cow<'a, str>>,
}

// Operations
impl AnniWorkspace {
    /// Add album to workspace
    ///
    /// `Untracked` -> `Committed`
    pub fn commit<P, V>(&self, path: P, validator: Option<V>) -> Result<Uuid, WorkspaceError>
    where
        P: AsRef<Path>,
        V: FnOnce(&[WorkspaceDisc]) -> bool,
    {
        let album = self.get_workspace_album(path.as_ref())?;
        let album_path = match album.state {
            // check current state of the album
            WorkspaceAlbumState::Untracked(p) => p,
            state => {
                return Err(WorkspaceError::InvalidAlbumState(state));
            }
        };
        let album_id = album.album_id;

        // validate album lock
        let lock = album_path.join(".album.lock");
        if lock.exists() {
            return Err(WorkspaceError::AlbumLocked(album_path));
        }

        // validate album cover
        let album_cover = album_path.join("cover.jpg");
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
            return Err(WorkspaceError::InvalidAlbumDiscStructure(album_path));
        }

        // add album as disc if there's only one disc
        if flac_in_album_root {
            discs.push(album_path.clone());
        }

        alphanumeric_sort::sort_path_slice(&mut discs);
        let discs = discs
            .into_iter()
            .enumerate()
            .map(|(index, disc)| {
                let index = index + 1;

                // iterate over all flac files
                let mut files = fs::read_dir(&disc)?
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

                let disc_cover = disc.join("cover.jpg");
                if !disc_cover.exists() {
                    return Err(WorkspaceError::CoverNotFound(disc_cover));
                }

                Ok(WorkspaceDisc {
                    index,
                    path: disc,
                    cover: disc_cover,
                    tracks: files,
                })
            })
            .collect::<Result<Vec<_>, WorkspaceError>>()?;

        if let Some(validator) = validator {
            let pass = validator(&discs);
            if !pass {
                return Err(WorkspaceError::UserAborted);
            }
        }

        // Add action
        // 1. lock album
        fs::File::create(&lock)?;

        // 2. copy or move album cover
        let album_controlled_path = self.get_album_controlled_path(&album_id)?;
        let album_cover_controlled = album_controlled_path.join("cover.jpg");
        if flac_in_album_root {
            // cover might be used by discs, copy it
            fs::copy(&album_cover, &album_cover_controlled)?;
        } else {
            // move directly
            fs::rename(&album_cover, &album_cover_controlled)?;
            fs::symlink_file(&album_cover_controlled, &album_cover)?;
        }

        // 3. move discs
        for disc in discs.iter() {
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
            let disc_cover_controlled_path = disc_controlled_path.join("cover.jpg");
            fs::rename(&disc.cover, &disc_cover_controlled_path)?;
            fs::symlink_file(&disc_cover_controlled_path, &disc.cover)?;
        }

        // 4. release lock
        fs::remove_file(lock, false)?;

        Ok(album_id)
    }

    /// Import tag from **committed** album.
    pub fn import_tags<P, E>(&self, album_path: P, extractor: E) -> Result<Uuid, WorkspaceError>
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
                DiscInfo::new(catalog.to_string(), None, None, None, Default::default()),
                tracks,
            ));
        }

        let album = Album::new(
            AlbumInfo {
                album_id,
                title: title.to_string(),
                edition: edition.map(|c| c.to_string()),
                artist: "UnknownArtist".to_string(),
                release_date,
                catalog: catalog.to_string(),
                ..Default::default()
            },
            discs,
        );
        repo.add_album(album, false)?;

        Ok(album_id)
    }

    pub fn revert<P>(&self, path: P)
    where
        P: AsRef<Path>,
    {
        //
    }
}
