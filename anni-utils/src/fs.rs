use std::path::PathBuf;
use std::fs::{read_dir};
use std::io;
pub use std::fs::metadata;

fn fs_walk_path(path: PathBuf, recursive: bool, callback: &impl Fn(PathBuf) -> bool) -> io::Result<bool> {
    let meta = metadata(&path)?;
    if meta.is_dir() && recursive {
        let mut dir: Vec<_> = read_dir(path)?.map(|r| r.unwrap().path()).collect();
        dir.sort();
        for entry in dir {
            if !fs_walk_path(entry, recursive, callback)? {
                return Ok(false);
            }
        }
        Ok(true)
    } else {
        Ok(callback(path))
    }
}

pub fn walk_path(path: PathBuf, recursive: bool, callback: impl Fn(PathBuf) -> bool) -> io::Result<()> {
    let _ = fs_walk_path(path, recursive, &callback)?;
    Ok(())
}