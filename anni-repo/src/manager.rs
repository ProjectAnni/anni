use crate::prelude::*;
use anni_common::traits::FromFile;
use anni_common::fs;
use std::path::{PathBuf, Path};
use std::collections::{HashMap, HashSet};

pub struct RepositoryManager {
    root: PathBuf,
    pub repo: Repository,

    /// All available tags.
    tags: HashSet<RepoTag>,
    /// Parent to child tag relation
    tags_relation: HashMap<TagRef, HashSet<TagRef>>,

    album_tags: HashMap<TagRef, Vec<String>>,

    albums: HashMap<String, Album>,
}

impl RepositoryManager {
    pub fn new<P: AsRef<Path>>(root: P) -> RepoResult<Self> {
        let repo = root.as_ref().join("repo.toml");
        let mut repo = Self {
            root: root.as_ref().to_owned(),
            repo: Repository::from_file(repo).map_err(Error::RepoInitError)?,
            tags: Default::default(),
            tags_relation: Default::default(),
            album_tags: Default::default(),
            albums: Default::default(),
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
    pub fn load_album(&self, catalog: &str) -> RepoResult<Album> {
        Album::from_file(self.album_path(catalog)).map_err(|e| Error::RepoAlbumLoadError {
            album: catalog.to_owned(),
            err: e,
        })
    }

    /// Add new album to the repository.
    pub fn add_album(&self, catalog: &str, album: Album) -> RepoResult<()> {
        let file = self.album_path(catalog);
        fs::write(&file, album.to_string())?;
        Ok(())
    }

    /// Open editor for album with given catalog.
    pub fn edit_album(&self, catalog: &str) -> RepoResult<()> {
        let file = self.album_path(catalog);
        edit::edit_file(&file)?;
        Ok(())
    }

    /// Get an iterator of available album catalogs in the repository.
    pub fn catalogs(&self) -> RepoResult<impl Iterator<Item=String>> {
        Ok(fs::read_dir(self.album_root())?
            .filter_map(|p| {
                let p = p.ok()?;
                if let Some("toml") = p.path().extension()?.to_str() {
                    p.path().file_stem().map(|f| f.to_string_lossy().to_string())
                } else { None }
            }))
    }

    pub fn albums(&self) -> impl Iterator<Item=&Album> {
        self.albums.values()
    }

    fn add_tag_relation(&mut self, parent: TagRef, child: TagRef) {
        if let Some(children) = self.tags_relation.get_mut(&parent) {
            children.insert(child);
        } else {
            let mut set = HashSet::new();
            set.insert(child);
            self.tags_relation.insert(parent, set);
        }
    }

    /// Load tags into self.tags.
    fn load_tags(&mut self) -> RepoResult<()> {
        // filter out toml files
        let tags_path = fs::PathWalker::new(self.root.join("tag"), true)
            .filter(|p| p.extension().map(|e| e == "toml").unwrap_or(false));

        // clear tags
        self.tags.clear();
        self.tags_relation.clear();

        // iterate over tag files
        for tag_file in tags_path {
            let text = fs::read_to_string(&tag_file)?;
            let tags = toml::from_str::<Tags>(&text).map_err(|e| crate::error::Error::TomlParseError {
                target: "Tags",
                input: text,
                err: e,
            })?.into_inner();

            for tag in tags {
                for parent in tag.parents() {
                    self.add_tag_relation(parent.clone(), tag.get_ref());
                }

                // add children to set
                for child in tag.children_raw() {
                    if !self.tags.insert(RepoTag::Full(child.clone().extend_simple(vec![tag.get_ref()]))) {
                        // duplicated simple tag
                        return Err(Error::RepoTagDuplicate {
                            tag: child.clone(),
                            path: tag_file,
                        });
                    }
                    self.add_tag_relation(tag.get_ref(), child.clone());
                }

                let tag_ref = tag.get_ref();
                if !self.tags.insert(RepoTag::Full(tag)) {
                    // duplicated
                    return Err(Error::RepoTagDuplicate {
                        tag: tag_ref,
                        path: tag_file,
                    });
                }
            }
        }

        // check tag relationship
        let all_tags: HashSet<_> = self.tags.iter().map(|t| t.get_ref()).collect();
        let all_tags: HashSet<_> = all_tags.iter().collect();
        let mut rel_tags: HashSet<_> = self.tags_relation.keys().collect();
        let rel_children: HashSet<_> = self.tags_relation.values().flatten().collect();
        rel_tags.extend(rel_children);
        if !rel_tags.is_subset(&all_tags) {
            return Err(Error::RepoTagsUndefined(rel_tags.difference(&all_tags).cloned().cloned().collect()));
        }

        Ok(())
    }

    /// Load albums tags.
    fn load_album_tags(&mut self) -> RepoResult<()> {
        self.album_tags.clear();

        for catalog in self.catalogs()? {
            let album = self.load_album(&catalog)?;
            let tags = album.tags();
            if tags.is_empty() {
                // this album has no tag
                log::warn!("No tag found in album {}, catalog = {}", album.album_id(), catalog);
            } else {
                for tag in tags {
                    if !self.tags.contains(&RepoTag::Ref(tag.clone())) {
                        log::warn!("Orphan tag {} found in album {}, catalog = {}", tag, album.album_id(), catalog);
                    }

                    if !self.album_tags.contains_key(tag) {
                        self.album_tags.insert(tag.clone(), vec![]);
                    }
                    self.album_tags.get_mut(tag).unwrap().push(catalog.clone());
                }
            }
            self.albums.insert(album.album_id().to_string(), album);
        }

        Ok(())
    }

    pub fn check_tags_loop(&self) -> bool {
        fn dfs<'tag, 'func>(
            tag: &'tag TagRef,
            tags_relation: &'tag HashMap<TagRef, HashSet<TagRef>>,
            current: &'func mut HashMap<&'tag TagRef, bool>,
            visited: &'func mut HashMap<&'tag TagRef, bool>,
            mut path: Vec<&'tag TagRef>,
        ) -> (bool, Vec<&'tag TagRef>) {
            visited.insert(tag, true);
            current.insert(tag, true);
            path.push(tag);

            if let Some(children) = tags_relation.get(tag) {
                for child in children {
                    if let Some(true) = current.get(child) {
                        path.push(child);
                        return (true, path);
                    }
                    // if !visited[child]
                    if !visited.get(child).map_or(false, |x| *x) {
                        let (loop_detected, loop_path) = dfs(child, tags_relation, current, visited, path);
                        if loop_detected {
                            return (true, loop_path);
                        } else {
                            path = loop_path;
                        }
                    }
                }
            }

            current.insert(tag, false);
            path.pop();
            (false, path)
        }

        let mut visited: HashMap<&TagRef, bool> = Default::default();
        let mut current: HashMap<&TagRef, bool> = Default::default();
        let tags: Vec<_> = self.tags.iter().map(|t| t.get_ref()).collect();
        for tag in tags.iter() {
            // if !visited[tag]
            if !visited.get(&tag).map_or(false, |x| *x) {
                let (loop_detected, path) = dfs(&tag, &self.tags_relation, &mut current, &mut visited, Default::default());
                if loop_detected {
                    // FIXME: return path, do not print here
                    log::error!("Loop detected: {:?}", path);
                    return false;
                }
            }
        }

        true
    }

    pub fn tags(&self) -> &HashSet<RepoTag> {
        &self.tags
    }
}
