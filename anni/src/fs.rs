use std::path::PathBuf;
use std::fs::{read_dir};
pub use std::fs::metadata;

fn fs_walk_path(path: PathBuf, recursive: bool, callback: impl Fn(PathBuf) -> bool + Copy) -> Result<bool, String> {
    let meta = metadata(&path).map_err(|e| e.to_string())?;
    if meta.is_dir() && recursive {
        let dir = read_dir(path).map_err(|e| e.to_string())?;
        for f in dir {
            let entry = f.map_err(|e| e.to_string())?;
            if !fs_walk_path(entry.path(), recursive, callback)? {
                return Ok(false);
            }
        }
        Ok(true)
    } else {
        Ok(callback(path))
    }
}

pub(crate) fn walk_path(path: PathBuf, recursive: bool, callback: impl Fn(PathBuf) -> bool + Copy) -> Result<(), String> {
    let _ = fs_walk_path(path, recursive, callback)?;
    Ok(())
}