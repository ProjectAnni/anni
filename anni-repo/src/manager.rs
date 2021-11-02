use crate::Result;
use crate::{Album, Repository};
use anni_common::traits::FromFile;
use anni_common::fs;
use std::path::{PathBuf, Path};
use std::collections::{HashMap, HashSet};
use crate::tag::{RepoTag, TagRef, Tags};

pub struct RepositoryManager {
    root: PathBuf,
    pub repo: Repository,

    /// All available tags.
    tags: HashSet<RepoTag>,
    /// Path -> Tags map.
    tags_by_file: HashMap<PathBuf, HashSet<TagRef>>,
    /// Top level tags with no parent.
    tags_top: HashSet<TagRef>,

    /// Albums with no tag
    album_without_tag: Vec<String>,
    album_tags: HashMap<TagRef, Vec<String>>,
}

impl RepositoryManager {
    pub fn new<P: AsRef<Path>>(root: P) -> Result<Self> {
        let repo = root.as_ref().join("repo.toml");
        let mut repo = Self {
            root: root.as_ref().to_owned(),
            repo: Repository::from_file(repo).map_err(crate::Error::RepoInitError)?,
            tags: Default::default(),
            tags_by_file: Default::default(),
            tags_top: Default::default(),
            album_tags: Default::default(),
            album_without_tag: Default::default(),
        };
        repo.load_tags()?;
        repo.load_album_tags()?;
        Ok(repo)
    }

    /// Return root path of albums in the repository.
    fn album_root(&self) -> PathBuf {
        self.root.join("album")
    }

    /// Return path of the album with given catalog.
    fn album_path(&self, catalog: &str) -> PathBuf {
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
        let tags_path = fs::PathWalker::new(self.root.join("tag"), true)
            .filter(|p| p.extension().map(|e| e == "toml").unwrap_or(false));

        // clear tags
        self.tags.clear();
        self.tags_by_file.clear();
        self.tags_top.clear();

        // iterate over tag files
        for tag_file in tags_path {
            let text = fs::read_to_string(&tag_file)?;
            let tags = toml::from_str::<Tags>(&text).map_err(|e| crate::error::Error::TomlParseError {
                target: "Tags",
                input: text,
                err: e,
            })?.into_inner();
            let tags_count = tags.len();

            let refs = tags.iter().map(|t| t.get_ref()).collect::<HashSet<_>>();
            let tags = tags.into_iter().map(|t| RepoTag::Full(t)).collect::<HashSet<_>>();

            if tags_count != tags.len() || !self.tags.is_disjoint(&tags) {
                return Err(crate::Error::RepoTagDuplicate(tag_file));
            } else {
                for tag in tags.iter() {
                    if let RepoTag::Full(tag) = tag {
                        // if parents is empty, add to top level tags set
                        if tag.parents().is_empty() {
                            self.tags_top.insert(tag.get_ref());
                        }

                        // add children to set
                        for child in tag.children_raw() {
                            self.tags.insert(RepoTag::Full(child.clone().extend_simple(vec![tag.get_ref()])));
                        }
                    } else {
                        unreachable!()
                    }
                }
                self.tags.extend(tags);
                let relative_path = tag_file.strip_prefix(self.root.join("tag")).expect("Failed to extract tag file relative path, this should not happen.");
                self.tags_by_file.insert(relative_path.to_path_buf(), refs);
            }
        }

        // check parent exists
        for tag in self.tags.iter() {
            if let RepoTag::Full(tag) = tag {
                for parent in tag.parents() {
                    if !self.tags.contains(&RepoTag::Ref(parent.clone())) {
                        return Err(crate::Error::RepoTagParentNotFound {
                            tag: tag.get_ref(),
                            parent: parent.clone(),
                        });
                    }
                }
            } else {
                unreachable!()
            }
        }

        Ok(())
    }

    /// Load albums tags.
    fn load_album_tags(&mut self) -> Result<()> {
        self.album_tags.clear();
        self.album_without_tag.clear();

        for catalog in self.catalogs()? {
            let album = self.load_album(&catalog)?;
            let tags = album.tags();
            if tags.is_empty() {
                // this album has no tag
                self.album_without_tag.push(catalog);
            } else {
                for tag in tags {
                    if !self.album_tags.contains_key(tag) {
                        self.album_tags.insert(tag.clone(), vec![]);
                    }
                    self.album_tags.get_mut(tag).unwrap().push(catalog.clone());
                }
            }
        }

        Ok(())
    }
}
