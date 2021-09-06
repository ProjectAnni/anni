use crate::Result;
use crate::{Album, Repository};
use crate::category::Category;
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
            repo: Repository::from_file(repo).map_err(crate::Error::RepoInitError)?,
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
        let file = self.with_album(catalog);
        fs::write(&file, album.to_string())?;
        Ok(())
    }

    pub fn edit_album(&self, catalog: &str) -> Result<()> {
        let file = self.with_album(catalog);
        edit::edit_file(&file)?;
        Ok(())
    }

    pub fn catalogs(&self) -> Result<impl Iterator<Item=String>> {
        Ok(fs::read_dir(self.album_root())?
            .filter_map(|p| {
                let p = p.ok()?;
                if let Some("toml") = p.path().extension()?.to_str() {
                    p.path().file_stem().map(|f| f.to_string_lossy().to_string())
                } else { None }
            }))
    }

    pub fn category_root(&self) -> PathBuf {
        self.root.join("category")
    }

    pub fn with_category(&self, catalog: &str) -> PathBuf {
        self.category_root().join(format!("{}.toml", catalog))
    }

    pub fn load_category(&self, category: &str) -> Result<Category> {
        Category::from_file(self.with_category(category)).map_err(|e| crate::Error::RepoCategoryLoadError {
            category: category.to_owned(),
            err: e,
        })
    }

    pub fn categories(&self) -> Result<impl Iterator<Item=String>> {
        Ok(fs::read_dir(self.category_root())?
            .filter_map(|p| {
                let path = p.ok()?.path();
                if let Some("toml") = path.extension()?.to_str() {
                    path.file_stem().map(|f| f.to_string_lossy().to_string())
                } else { None }
            }))
    }
}
