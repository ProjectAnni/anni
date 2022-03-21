use crate::prelude::*;
use anni_common::fs;
use std::collections::{HashMap, HashSet};
use std::path::{PathBuf, Path};
use std::str::FromStr;

pub struct RepositoryManager {
    root: PathBuf,
    pub repo: Repository,
}

impl RepositoryManager {
    pub fn new<P: AsRef<Path>>(root: P) -> RepoResult<Self> {
        let repo = root.as_ref().join("repo.toml");
        Ok(Self {
            root: root.as_ref().to_owned(),
            repo: Repository::from_str(&fs::read_to_string(repo)?)?,
        })
    }

    #[cfg(feature = "git")]
    pub fn clone<P: AsRef<Path>>(url: &str, root: P, branch: &str) -> RepoResult<Self> {
        if root.as_ref().exists() {
            // pull instead of clone
            crate::utils::git::pull(root.as_ref(), branch)?;
        } else {
            git2::Repository::clone(url, root.as_ref())?;
        }
        Self::new(root.as_ref())
    }

    pub fn name(&self) -> &str {
        self.repo.name()
    }

    pub fn edition(&self) -> &str {
        self.repo.edition()
    }

    // Get all album roots.
    fn album_roots(&self) -> Vec<PathBuf> {
        self.repo
            .albums()
            .iter()
            .map(|album| self.root.join(album))
            .collect()
    }

    fn default_album_root(&self) -> PathBuf {
        self.root.join(self.repo.albums().get(0).map_or_else(|| "album", String::as_str))
    }

    /// Get all album paths.
    /// TODO: use iterator
    pub fn all_album_paths(&self) -> RepoResult<Vec<PathBuf>> {
        let mut paths = Vec::new();
        for root in self.album_roots() {
            let files = fs::read_dir(root)?;
            for file in files {
                let file = file?;
                let path = file.path();
                if path.is_file() {
                    paths.push(path);
                } else if path.is_dir() {
                    let mut index = 0;
                    let catalog = file.file_name();
                    loop {
                        let path = path.join(&catalog).with_extension(format!("{index}.toml"));
                        if path.exists() {
                            paths.push(path);
                            index += 1;
                        } else {
                            break;
                        }
                    }
                }
            }
        }
        Ok(paths)
    }

    /// Get album paths with given catalog.
    pub fn album_paths(&self, catalog: &str) -> RepoResult<Vec<PathBuf>> {
        let mut paths = Vec::new();
        for root in self.album_roots() {
            let file = root.join(format!("{catalog}.toml"));
            if file.exists() {
                // toml exists
                paths.push(file);
            } else {
                let folder = root.join(catalog);
                if folder.exists() {
                    // folder /{catalog} exists
                    for file in fs::read_dir(folder)? {
                        let dir = file?;
                        if dir.path().extension() == Some("toml".as_ref()) {
                            paths.push(dir.path());
                        }
                    }
                }
            }
        }
        Ok(paths)
    }

    /// Load album with given path.
    fn load_album<P>(&self, path: P) -> RepoResult<Album>
        where P: AsRef<Path> {
        let input = fs::read_to_string(path.as_ref())?;
        Album::from_str(&input)
    }

    /// Load album(s) with given catalog.
    pub fn load_albums(&self, catalog: &str) -> anyhow::Result<Vec<Album>> {
        Ok(self
            .album_paths(catalog)?
            .into_iter()
            .filter_map(|path| {
                let album = self.load_album(&path);
                match album {
                    Ok(album) => Some(album),
                    Err(err) => {
                        log::error!("Failed to load album in {path:?}: {err}",);
                        None
                    }
                }
            })
            .collect())
    }

    /// Add new album to the repository.
    pub fn add_album(&self, catalog: &str, album: Album, allow_duplicate: bool) -> RepoResult<()> {
        let folder = self.default_album_root().join(catalog);
        let file = folder.with_extension("toml");

        if folder.exists() {
            // multiple albums with the same catalog exists
            let count = fs::PathWalker::new(&folder, false).filter(|p|
                // p.extension is toml
                p.extension() == Some("toml".as_ref())
            ).count();
            let new_file_name = format!("{catalog}.{count}.toml");
            fs::write(folder.join(new_file_name), album.to_string())?;
        } else if file.exists() {
            // album with the same catalog exists
            if !allow_duplicate {
                return Err(Error::RepoAlbumExists(catalog.to_string()));
            }
            // make sure the folder exists
            fs::create_dir_all(&folder)?;
            // move the old toml file to folder
            fs::rename(file, folder.join(format!("{catalog}.0.toml")))?;
            // write new toml file
            fs::write(folder.join(format!("{catalog}.1.toml")), album.to_string())?;
        } else {
            // no catalog with given catalog exists
            fs::write(&file, album.to_string())?;
        }
        Ok(())
    }

    /// Open editor for album with given catalog.
    pub fn edit_album(&self, catalog: &str) -> RepoResult<()> {
        for file in self.album_paths(catalog)? {
            edit::edit_file(&file)?;
        }
        Ok(())
    }

    pub fn into_owned_manager(self) -> RepoResult<OwnedRepositoryManager> {
        OwnedRepositoryManager::new(self)
    }
}

/// A repository manager which own full copy of a repo.
///
/// This is helpful when you need to perform a full-repo operation,
/// such as ring check on tags, full-repo validation, etc.
pub struct OwnedRepositoryManager {
    pub repo: RepositoryManager,

    /// All available tags.
    tags: HashSet<RepoTag>,
    /// Parent to child tag relation
    tags_relation: HashMap<TagRef, HashSet<TagRef>>,

    album_tags: HashMap<TagRef, Vec<String>>,

    albums: HashMap<String, Album>,
}

impl<'repo> OwnedRepositoryManager {
    pub fn new(repo: RepositoryManager) -> RepoResult<Self> {
        let mut repo = Self {
            repo,
            tags: Default::default(),
            tags_relation: Default::default(),
            album_tags: Default::default(),
            albums: Default::default(),
        };
        repo.load_tags()?;
        repo.load_album_tags()?;
        Ok(repo)
    }

    pub fn albums(&self) -> impl Iterator<Item=&Album> {
        self.albums.values()
    }

    pub fn tags(&self) -> &HashSet<RepoTag> {
        &self.tags
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
        let tags_path = fs::PathWalker::new(self.repo.root.join("tag"), true)
            .filter(|p| p.extension().map(|e| e == "toml").unwrap_or(false));

        // clear tags
        self.tags.clear();
        self.tags_relation.clear();

        // iterate over tag files
        for tag_file in tags_path {
            let text = fs::read_to_string(&tag_file)?;
            let tags = toml::from_str::<Tags>(&text)
                .map_err(|e| crate::error::Error::TomlParseError {
                    target: "Tags",
                    input: text,
                    err: e,
                })?
                .into_inner();

            for tag in tags {
                for parent in tag.parents() {
                    self.add_tag_relation(parent.clone(), tag.get_ref());
                }

                // add children to set
                for child in tag.children_raw() {
                    if !self.tags.insert(RepoTag::Full(
                        child.clone().extend_simple(vec![tag.get_ref()]),
                    )) {
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
            return Err(Error::RepoTagsUndefined(
                rel_tags.difference(&all_tags).cloned().cloned().collect(),
            ));
        }

        Ok(())
    }

    /// Load albums tags.
    fn load_album_tags(&mut self) -> RepoResult<()> {
        self.album_tags.clear();

        let mut has_problem = false;
        for path in self.repo.all_album_paths()? {
            let album = self.repo.load_album(path)?;
            let catalog = album.catalog();
            let tags = album.tags();
            if tags.is_empty() {
                // this album has no tag
                log::warn!(
                    "No tag found in album {}, catalog = {}",
                    album.album_id(),
                    catalog,
                );
                has_problem = true;
            } else {
                for tag in tags {
                    if !self.tags.contains(&RepoTag::Ref(tag.clone())) {
                        log::error!(
                            "Orphan tag {} found in album {}, catalog = {}",
                            tag,
                            album.album_id(),
                            catalog
                        );
                        has_problem = true;
                    }

                    if !self.album_tags.contains_key(tag) {
                        self.album_tags.insert(tag.clone(), vec![]);
                    }
                    self.album_tags
                        .get_mut(tag)
                        .unwrap()
                        .push(catalog.to_string());
                }
            }
            self.albums.insert(album.album_id().to_string(), album);
        }

        if !has_problem {
            Ok(())
        } else {
            Err(Error::RepoInitError(anyhow::anyhow!(
                "Problems detected in album tags"
            )))
        }
    }

    pub fn check_tags_loop(&self) -> Option<Vec<TagRef>> {
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
                        let (loop_detected, loop_path) =
                            dfs(child, tags_relation, current, visited, path);
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
                let (loop_detected, path) = dfs(
                    &tag,
                    &self.tags_relation,
                    &mut current,
                    &mut visited,
                    Default::default(),
                );
                if loop_detected {
                    return Some(path.into_iter().map(|p| p.clone()).collect());
                }
            }
        }

        None
    }

    #[cfg(feature = "db")]
    pub async fn to_database<P>(&self, database_path: P) -> RepoResult<()>
        where P: AsRef<Path> {
        use std::time::{SystemTime, UNIX_EPOCH};

        // remove database first
        let _ = std::fs::remove_file(database_path.as_ref());

        let mut db = crate::db::RepoDatabaseWrite::create(database_path.as_ref()).await?;
        // TODO: get url / ref from repo
        db.write_info(self.repo.name(), self.repo.edition(), "", "").await?;

        // Write all tags
        let tags = self.tags().iter().filter_map(|t| match t {
            RepoTag::Full(tag) => Some(tag),
            _ => None,
        });
        db.add_tags(tags).await?;

        // Write all albums
        for album in self.albums() {
            db.add_album(album).await?;
        }

        // Create Index
        db.create_index().await?;


        // Creation time
        fs::write(database_path.as_ref().with_file_name("repo.json"), &format!("{{\"last_modified\": {}}}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()))?;
        Ok(())
    }
}
