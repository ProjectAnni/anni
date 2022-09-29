use crate::workspace::{WorkspaceAlbum, WorkspaceAlbumState};
use anni_common::fs;
use anni_provider::strict_album_path;
use anni_repo::library::file_name;
use anni_repo::RepositoryManager;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use uuid::Uuid;

pub fn find_dot_anni() -> anyhow::Result<PathBuf> {
    let path = std::env::current_dir()?;

    let mut path = path.as_path();
    loop {
        let dot_anni = path.join(".anni");
        if dot_anni.exists() {
            let config_path = dot_anni.join("config.toml");
            if config_path.exists() {
                return Ok(dot_anni);
            } else {
                log::warn!(
                    "dot anni directory was detected at {}, but config.toml does not exist",
                    dot_anni.display()
                );
            }
        }
        path = path.parent().ok_or_else(|| {
            anyhow::anyhow!("Could not find .anni in current directory or any parent")
        })?;
    }
}

pub fn find_workspace_root() -> anyhow::Result<PathBuf> {
    find_dot_anni().map(|mut p| {
        p.pop();
        p
    })
}

pub fn get_workspace_album_real_path<P>(root: P, path: P) -> anyhow::Result<PathBuf>
where
    P: AsRef<Path>,
{
    let album_id = get_album_id(path.as_ref())?;
    match album_id {
        Some(album_id) => Ok(get_workspace_album_path(root, &album_id)
            .ok_or_else(|| anyhow::anyhow!("Album directory does not exist"))?),
        None => bail!("Album directory does not exist, or is not a symlink"),
    }
}

pub fn get_workspace_album_path<P>(dot_anni: P, album_id: &Uuid) -> Option<PathBuf>
where
    P: AsRef<Path>,
{
    let path = strict_album_path(&dot_anni.as_ref().join("objects"), &album_id.to_string(), 2);
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

/// Get album id from symlink target
///
/// Returns `None` if the symlink does not exist, or is not a symlink
/// Returns `Error` only if album id is not a valid uuid
pub fn get_album_id<P>(path: P) -> anyhow::Result<Option<Uuid>>
where
    P: AsRef<Path>,
{
    let album_path = path.as_ref().join(".album");

    // 1. validate album path
    // if it does not exist, or is not a symlink, return None
    if !album_path.is_symlink() {
        return Ok(None);
    }

    // 2. get album_id
    let real_path = fs::read_link(album_path)?;
    let album_id = real_path.file_name().unwrap().to_string_lossy();
    let album_id = Uuid::parse_str(&album_id)?;
    Ok(Some(album_id))
}

pub fn get_workspace_repository_manager<P>(dot_anni: P) -> anyhow::Result<RepositoryManager>
where
    P: AsRef<Path>,
{
    let root = dot_anni.as_ref().join("repo");
    Ok(RepositoryManager::new(root)?)
}

pub fn scan_workspace<P>(root: P) -> anyhow::Result<Vec<WorkspaceAlbum>>
where
    P: AsRef<Path>,
{
    fn scan_workspace_userland_directory<P1, P2>(
        albums: &mut HashMap<Uuid, WorkspaceAlbum>,
        dot_anni: P1,
        path: P2,
    ) -> anyhow::Result<()>
    where
        P1: AsRef<Path>,
        P2: AsRef<Path>,
    {
        for entry in fs::read_dir(path.as_ref())? {
            let entry = entry?;
            if entry.file_name() == ".anni" {
                continue;
            }

            let metadata = entry.metadata()?;
            if metadata.is_dir() {
                // look for .album folder
                match get_album_id(entry.path())? {
                    // valid album_id, it's an album directory
                    Some(album_id) => {
                        let album_controlled_path = get_workspace_album_path(&dot_anni, &album_id)
                            .and_then(|p| if p.exists() { Some(p) } else { None });
                        albums.insert(
                            album_id.clone(),
                            WorkspaceAlbum {
                                album_id,
                                state: match album_controlled_path {
                                    Some(controlled_path) => {
                                        if fs::read_dir(controlled_path)?.next().is_some() {
                                            WorkspaceAlbumState::Committed(entry.path())
                                        } else {
                                            WorkspaceAlbumState::Untracked(entry.path())
                                        }
                                    }
                                    None => WorkspaceAlbumState::Dangling(entry.path()),
                                },
                            },
                        );
                    }
                    // symlink was not found, scan recursively
                    None => {
                        scan_workspace_userland_directory(albums, dot_anni.as_ref(), &entry.path())?
                    }
                }
            }
        }

        Ok(())
    }

    fn scan_workspace_controlled_directory<P>(
        albums: &mut HashMap<Uuid, WorkspaceAlbum>,
        parent: P,
        level: u8,
    ) -> anyhow::Result<()>
    where
        P: AsRef<Path>,
    {
        let parent = parent.as_ref();
        for entry in fs::read_dir(parent)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                if level > 0 {
                    scan_workspace_controlled_directory(albums, path, level - 1)?;
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

    let mut albums = HashMap::new();
    scan_workspace_userland_directory(&mut albums, root.as_ref().join(".anni"), &root)?;
    scan_workspace_controlled_directory(&mut albums, root.as_ref().join(".anni/objects"), 2)?;
    Ok(albums.into_iter().map(|r| r.1).collect())
}
