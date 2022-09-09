use anni_common::fs;
use anni_provider::strict_album_path;
use std::path::{Path, PathBuf};
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

pub fn get_workspace_album_real_path<P>(root: P, path: P) -> anyhow::Result<PathBuf>
where
    P: AsRef<Path>,
{
    let album_path = path.as_ref().join(".album");

    // 1. find .album
    if !album_path.is_symlink() {
        bail!("Album directory is not a symlink");
    }

    // 2. get album_id
    let real_path = fs::read_link(&album_path)?;
    let album_id = real_path.file_name().unwrap().to_string_lossy();
    if Uuid::parse_str(&album_id).is_err() {
        bail!("Invalid album id detected");
    }

    // 3. return album_id
    Ok(strict_album_path(
        &root.as_ref().join("objects"),
        &album_id,
        2,
    ))
}
