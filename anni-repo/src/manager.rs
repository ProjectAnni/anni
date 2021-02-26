use std::path::{PathBuf, Path};
use crate::{Repository, Album};
use std::fs;
use anni_common::FromFile;
use crate::Result;

pub struct RepositoryManager {
    root: PathBuf,
    pub repo: Repository,
}

impl RepositoryManager {
    pub fn new<S: AsRef<str>>(root: S) -> Result<Self> {
        let root = Path::new(root.as_ref());
        let repo = root.join("repo.toml");
        Ok(Self {
            root: root.to_owned(),
            repo: Repository::from_file(repo).map_err(|e| crate::Error::RepoInitError(e))?,
        })
    }

    pub fn album_root(&self) -> PathBuf {
        self.root.join("album")
    }

    pub fn with_album(&self, catalog: &str) -> PathBuf {
        self.album_root().join(format!("{}.toml", catalog))
    }

    pub fn album_exists(&self, catalog: &str) -> bool {
        fs::metadata(self.with_album(catalog)).is_ok()
    }

    pub fn load_album(&self, catalog: &str) -> Result<Album> {
        Album::from_file(self.with_album(catalog)).map_err(|e| crate::Error::RepoAlbumLoadError {
            album: catalog.to_owned(),
            err: e,
        })
    }
}