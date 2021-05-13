use crate::Result;
use crate::{Album, Repository};
use anni_common::traits::FromFile;
use std::fs;
use std::path::{PathBuf, Path};

pub struct RepositoryManager {
    root: PathBuf,
    pub repo: Repository,
}

impl RepositoryManager {
    pub fn new<P: AsRef<Path>>(root: P) -> Result<Self> {
        let repo = root.as_ref().join("repo.toml");
        Ok(Self {
            root: root.as_ref().to_owned(),
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

    pub fn add_album(&self, catalog: &str, album: Album) -> Result<()> {
        let file = self.with_album(&catalog);
        fs::write(&file, album.to_string())?;
        Ok(())
    }

    pub fn edit_album(&self, catalog: &str) -> Result<()> {
        let file = self.with_album(&catalog);
        edit::edit_file(&file)?;
        Ok(())
    }
}
