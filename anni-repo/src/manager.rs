use crate::prelude::*;
use anni_common::fs;
use indexmap::IndexSet;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use toml_edit::easy as toml;

/// A simple repository visitor. Can perform simple operations on the repository.
pub struct RepositoryManager {
    root: PathBuf,
    repo: Repository,
}

impl RepositoryManager {
    pub fn new<P: AsRef<Path>>(root: P) -> RepoResult<Self> {
        let repo = root.as_ref().join("repo.toml");

        #[cfg(feature = "git")]
        crate::utils::git::setup_git2_internal();

        Ok(Self {
            root: root.as_ref().to_owned(),
            repo: Repository::from_str(&fs::read_to_string(repo)?)?,
        })
    }

    #[cfg(feature = "git")]
    pub fn clone<P: AsRef<Path>>(url: &str, root: P) -> RepoResult<Self> {
        crate::utils::git::setup_git2_internal();
        git2::Repository::clone(url, root.as_ref())?;
        Self::new(root.as_ref())
    }

    #[cfg(feature = "git")]
    pub fn pull<P: AsRef<Path>>(root: P, branch: &str) -> RepoResult<Self> {
        crate::utils::git::setup_git2_internal();
        crate::utils::git::pull(root.as_ref(), branch)?;
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
        self.root.join(
            self.repo
                .albums()
                .get(0)
                .map_or_else(|| "album", String::as_str),
        )
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
    where
        P: AsRef<Path>,
    {
        let input = fs::read_to_string(path.as_ref())?;
        Album::from_str(&input)
    }

    /// Load album(s) with given catalog.
    pub fn load_albums(&self, catalog: &str) -> RepoResult<Vec<Album>> {
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
    pub fn add_album(&self, mut album: Album, allow_duplicate: bool) -> RepoResult<()> {
        let catalog = album.catalog();
        let folder = self.default_album_root().join(catalog);
        let file = folder.with_extension("toml");

        if folder.exists() {
            // multiple albums with the same catalog exists
            let count = fs::PathWalker::new(&folder, false, false, Default::default())
                .filter(|p|
                    // p.extension is toml
                    p.extension() == Some("toml".as_ref()))
                .count();
            let new_file_name = format!("{catalog}.{count}.toml");
            fs::write(folder.join(new_file_name), album.format_to_string())?;
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
            fs::write(
                folder.join(format!("{catalog}.1.toml")),
                album.format_to_string(),
            )?;
        } else {
            // no catalog with given catalog exists
            fs::write(&file, album.format_to_string())?;
        }
        Ok(())
    }

    pub fn into_owned_manager(self) -> RepoResult<OwnedRepositoryManager> {
        OwnedRepositoryManager::new(self)
    }

    pub fn root(&self) -> &Path {
        self.root.as_path()
    }
}

/// A repository manager which own full copy of a repo.
///
/// This is helpful when you need to perform a full-repo operation,
/// such as ring check on tags, full-repo validation, etc.
pub struct OwnedRepositoryManager {
    pub repo: RepositoryManager,

    /// All available tags.
    tags: HashSet<Tag>,
    /// Parent to child tag relation
    tags_relation: HashMap<TagRef<'static>, IndexSet<TagRef<'static>>>,
    /// Tag -> File
    tag_path: HashMap<TagRef<'static>, PathBuf>,

    album_tags: HashMap<TagRef<'static>, Vec<String>>,
    /// AlbumID -> Album
    albums: HashMap<String, Album>,
    /// AlbumID -> Album Path
    album_path: HashMap<String, PathBuf>,
}

impl OwnedRepositoryManager {
    pub fn new(repo: RepositoryManager) -> RepoResult<Self> {
        let mut repo = Self {
            repo,
            tags: Default::default(),
            tags_relation: Default::default(),
            tag_path: Default::default(),
            album_tags: Default::default(),
            albums: Default::default(),
            album_path: Default::default(),
        };
        repo.load_tags()?;
        repo.load_album_tags()?;
        Ok(repo)
    }

    pub fn album(&self, album_id: &str) -> Option<&Album> {
        self.albums.get(album_id)
    }

    pub fn album_path(&self, album_id: &str) -> Option<&Path> {
        self.album_path.get(album_id).map(|p| p.as_path())
    }

    pub fn albums(&self) -> &HashMap<String, Album> {
        &self.albums
    }

    pub fn albums_iter(&self) -> impl Iterator<Item = &Album> {
        self.albums.values()
    }

    pub fn tag(&self, tag: &TagRef<'_>) -> Option<&Tag> {
        self.tags.get(tag)
    }

    pub fn tags(&self) -> &HashSet<Tag> {
        &self.tags
    }

    pub fn tag_path<'a>(&'a self, tag: &'a TagRef<'_>) -> Option<&'a PathBuf> {
        self.tag_path.get(tag)
    }

    pub fn child_tags<'me, 'tag>(&'me self, tag: &TagRef<'tag>) -> IndexSet<&'me TagRef<'tag>>
    where
        'tag: 'me,
    {
        self.tags_relation
            .get(tag)
            .map_or(IndexSet::new(), |children| children.iter().collect())
    }

    pub fn albums_tagged_by<'me, 'tag>(
        &'me self,
        tag: &'me TagRef<'tag>,
    ) -> Option<&'me Vec<String>>
    where
        'tag: 'me,
    {
        self.album_tags.get(tag)
    }

    fn add_tag_relation(&mut self, parent: TagRef<'static>, child: TagRef<'static>) {
        if let Some(children) = self.tags_relation.get_mut(&parent) {
            children.insert(child);
        } else {
            let mut set = IndexSet::new();
            set.insert(child);
            self.tags_relation.insert(parent, set);
        }
    }

    /// Load tags into self.tags.
    fn load_tags(&mut self) -> RepoResult<()> {
        // filter out toml files
        let tags_path =
            fs::PathWalker::new(self.repo.root.join("tag"), true, false, Default::default())
                .filter(|p| p.extension().map(|e| e == "toml").unwrap_or(false));

        // clear tags
        self.tags.clear();
        self.tags_relation.clear();

        // iterate over tag files
        for tag_file in tags_path {
            let text = fs::read_to_string(&tag_file)?;
            let tags = toml::from_str::<Tags>(&text)
                .map_err(|e| Error::TomlParseError {
                    target: "Tags",
                    input: text,
                    err: e,
                })?
                .into_inner();
            let relative_path = pathdiff::diff_paths(&tag_file, &self.repo.root).unwrap();

            for tag in tags {
                for parent in tag.parents() {
                    self.add_tag_relation(parent.0.clone(), tag.get_owned_ref());
                }

                // add children to set
                for child in tag.children_simple() {
                    let parent = tag.get_owned_ref();
                    let full = child.clone().into_full(vec![parent.into()]);
                    if !self.tags.insert(full) {
                        // duplicated simple tag
                        return Err(Error::RepoTagDuplicate {
                            tag: child.clone(),
                            path: tag_file,
                        });
                    }
                    self.add_tag_relation(tag.get_owned_ref(), child.clone());
                    self.tag_path.insert(child.clone(), relative_path.clone());
                }

                let tag_ref = tag.get_owned_ref();
                if !self.tags.insert(tag) {
                    // duplicated
                    return Err(Error::RepoTagDuplicate {
                        tag: tag_ref,
                        path: tag_file,
                    });
                }

                self.tag_path.insert(tag_ref, relative_path.clone());
            }
        }

        // check tag relationship
        let all_tags: HashSet<_> = self.tags.iter().map(|t| t.get_owned_ref()).collect();
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

        let mut problems = vec![];
        for path in self.repo.all_album_paths()? {
            let album = self.repo.load_album(&path)?;
            let album_id = album.album_id();
            let catalog = album.catalog();
            let tags = album.tags();
            if tags.is_empty() {
                // this album has no tag
                log::warn!(
                    "No tag found in album {}, catalog = {}",
                    album.album_id(),
                    catalog,
                );
            } else {
                for tag in tags {
                    if !self.tags.contains(tag) {
                        log::error!(
                            "Orphan tag {} found in album {}, catalog = {}",
                            tag,
                            album_id.to_string(),
                            catalog
                        );
                        problems.push(Error::RepoTagsUndefined(vec![tag.clone()]));
                    }

                    if !self.album_tags.contains_key(tag) {
                        self.album_tags.insert(tag.clone(), vec![]);
                    }
                    self.album_tags
                        .get_mut(tag)
                        .unwrap()
                        .push(album_id.to_string());
                }
            }
            if let Some(album_with_same_id) = self.albums.insert(album_id.to_string(), album) {
                log::error!(
                    "Duplicated album id detected: {}",
                    album_with_same_id.album_id()
                );
                problems.push(Error::RepoDuplicatedAlbumId(album_id.to_string()));
            }
            self.album_path.insert(
                album_id.to_string(),
                pathdiff::diff_paths(&path, &self.repo.root).unwrap(),
            );
        }

        if problems.is_empty() {
            Ok(())
        } else {
            Err(Error::MultipleErrors(problems))
        }
    }

    pub fn check_tags_loop<'me, 'tag>(&'me self) -> Option<Vec<&'me TagRef<'tag>>>
    where
        'me: 'tag,
    {
        fn dfs<'tag, 'func>(
            tag: &'tag TagRef<'tag>,
            tags_relation: &'tag HashMap<TagRef<'static>, IndexSet<TagRef<'static>>>,
            current: &'func mut HashMap<&'tag TagRef<'tag>, bool>,
            visited: &'func mut HashMap<&'tag TagRef<'tag>, bool>,
            mut path: Vec<&'tag TagRef<'tag>>,
        ) -> (bool, Vec<&'tag TagRef<'tag>>) {
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
        for tag in tags.into_iter() {
            // if !visited[tag]
            if !visited.get(&tag).map_or(false, |x| *x) {
                let (loop_detected, path) = dfs(
                    tag,
                    &self.tags_relation,
                    &mut current,
                    &mut visited,
                    Default::default(),
                );
                if loop_detected {
                    return Some(path);
                }
            }
        }

        None
    }

    #[cfg(feature = "db-write")]
    pub fn to_database<P>(&self, database_path: P) -> RepoResult<()>
    where
        P: AsRef<Path>,
    {
        use std::time::{SystemTime, UNIX_EPOCH};

        // remove database first
        let _ = std::fs::remove_file(database_path.as_ref());

        let db = crate::db::RepoDatabaseWrite::create(database_path.as_ref())?;
        // TODO: get url / ref from repo
        db.write_info(self.repo.name(), self.repo.edition(), "", "")?;

        // Write all tags
        let tags = self.tags().iter();
        db.add_tags(tags)?;

        // Write all albums
        for album in self.albums_iter() {
            db.add_album(album)?;
        }

        // Create Index
        db.create_index()?;

        // Creation time
        fs::write(
            database_path.as_ref().with_file_name("repo.json"),
            format!(
                "{{\"last_modified\": {}}}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            ),
        )?;
        Ok(())
    }
}
