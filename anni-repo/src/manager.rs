use crate::Result;
use crate::{Album, Repository};
use anni_common::traits::FromFile;
use std::fs;
use std::path::{PathBuf, Path};
use std::collections::{HashMap, HashSet};
use crate::tag::{Tag, TagRef, Tags};

pub struct RepositoryManager {
    root: PathBuf,
    pub repo: Repository,

    /// All available tags.
    tags: HashMap<TagRef, Tag>,
    /// Path -> Tags map.
    tags_by_file: HashMap<PathBuf, HashSet<TagRef>>,
}

impl RepositoryManager {
    pub fn new<P: AsRef<Path>>(root: P) -> Result<Self> {
        let repo = root.as_ref().join("repo.toml");
        let mut repo = Self {
            root: root.as_ref().to_owned(),
            repo: Repository::from_file(repo).map_err(crate::Error::RepoInitError)?,
            tags: Default::default(),
            tags_by_file: Default::default(),
        };
        repo.load_tags()?;
        Ok(repo)
    }

    /// Return root path of albums in the repository.
    fn album_root(&self) -> PathBuf {
        self.root.join("album")
    }

    /// Return path of the album with given catalog.
    pub fn album_path(&self, catalog: &str) -> PathBuf {
        self.album_root().join(format!("{}.toml", catalog))
    }

    /// Check if the album with given catalog exists.
    pub fn album_exists(&self, catalog: &str) -> bool {
        fs::metadata(self.album_path(catalog)).is_ok()
    }

    /// Load album with given catalog.
    pub fn load_album(&self, catalog: &str) -> Result<Album> {
        Album::from_file(self.album_path(catalog)).map_err(|e| crate::Error::RepoAlbumLoadError {
            album: catalog.to_owned(),
            err: e,
        })
    }

    /// Add new album to the repository.
    pub fn add_album(&self, catalog: &str, album: Album) -> Result<()> {
        let file = self.album_path(catalog);
        fs::write(&file, album.to_string())?;
        Ok(())
    }

    /// Open editor for album with given catalog.
    pub fn edit_album(&self, catalog: &str) -> Result<()> {
        let file = self.album_path(catalog);
        edit::edit_file(&file)?;
        Ok(())
    }

    /// Get an iterator of available album catalogs in the repository.
    pub fn catalogs(&self) -> Result<impl Iterator<Item=String>> {
        Ok(fs::read_dir(self.album_root())?
            .filter_map(|p| {
                let p = p.ok()?;
                if let Some("toml") = p.path().extension()?.to_str() {
                    p.path().file_stem().map(|f| f.to_string_lossy().to_string())
                } else { None }
            }))
    }

    /// Load tags into self.tags.
    fn load_tags(&mut self) -> Result<()> {
        // filter out toml files
        let tags_path = fs::read_dir(self.root.join("tag"))?
            .filter_map(|p| {
                let path = p.ok()?.path();
                if let Some("toml") = path.extension()?.to_str() {
                    Some(path)
                } else {
                    None
                }
            });

        // clear tags
        self.tags.clear();
        self.tags_by_file.clear();

        // iterate over tag files
        for tag_file in tags_path {
            let text = anni_common::fs::read_to_string(&tag_file)?;
            let tags = toml::from_str::<Tags>(&text).map_err(|e| crate::error::Error::TomlParseError {
                target: "Tags",
                input: text,
                err: e,
            })?.into_inner();
            let refs = tags.iter().map(|t| t.get_ref()).collect::<HashSet<_>>();

            // iterate tags in the file
            for tag in tags {
                let tag_ref = tag.get_ref();
                if self.tags.contains_key(&tag_ref) {
                    return Err(crate::error::Error::RepoTagDuplicate(tag_ref));
                }
                self.tags.insert(tag_ref, tag);
            }
            self.tags_by_file.insert(tag_file, refs);
        }

        Ok(())
    }

    /// Load albums into self.albums.
    fn load_albums(&mut self) -> Result<()> {
        Ok(())
    }
}
