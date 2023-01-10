pub mod config;
mod error;
mod state;

use crate::config::WorkspaceConfig;
use anni_common::fs;
use anni_repo::library::file_name;
use anni_repo::prelude::RepoResult;
use anni_repo::RepositoryManager;
pub use error::WorkspaceError;
pub use state::*;
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
                    Err(WorkspaceError::ConfigTomlNotFound(config_path))
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
                match self.get_album_id(entry.path()) {
                    // valid album_id, it's an album directory
                    Ok(album_id) => {
                        let album_controlled_path = self.get_album_controlled_path(&album_id);
                        albums.insert(
                            album_id,
                            WorkspaceAlbum {
                                album_id,
                                state: match album_controlled_path {
                                    Ok(controlled_path) => {
                                        if !entry.path().join(".album").exists() {
                                            // symlink is broken
                                            WorkspaceAlbumState::Dangling(entry.path())
                                        } else if fs::read_dir(controlled_path)?.next().is_some() {
                                            // controlled part is not empty
                                            WorkspaceAlbumState::Committed(entry.path())
                                        } else {
                                            // controlled part is empty
                                            WorkspaceAlbumState::Untracked(entry.path())
                                        }
                                    }
                                    // controlled part does not exist
                                    Err(WorkspaceError::AlbumNotFound(_)) => {
                                        WorkspaceAlbumState::Dangling(entry.path())
                                    }
                                    _ => unreachable!(),
                                },
                            },
                        );
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
                    albums.entry(album_id).or_insert_with(|| WorkspaceAlbum {
                        album_id,
                        state: WorkspaceAlbumState::Garbage,
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

    pub fn to_repository_manager(&self) -> RepoResult<RepositoryManager> {
        RepositoryManager::new(self.repo_root())
    }

    pub fn get_config(&self) -> Result<WorkspaceConfig, WorkspaceError> {
        WorkspaceConfig::new(&self.dot_anni)
    }
}
