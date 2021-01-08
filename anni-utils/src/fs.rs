use std::path::{PathBuf, Path};
use std::fs::{read_dir};
use std::io;
pub use std::fs::metadata;

pub struct PathWalker {
    path: Vec<PathBuf>,
    files: Vec<PathBuf>,
    recursive: bool,
}

impl Iterator for PathWalker {
    type Item = PathBuf;

    fn next(&mut self) -> Option<Self::Item> {
        if self.files.is_empty() {
            if self.path.is_empty() || !self.recursive {
                return None;
            }
            while self.files.is_empty() && !self.path.is_empty() {
                self.extract_path();
            }
            if self.files.is_empty() {
                return None;
            }
        }

        Some(self.files.remove(0))
    }
}

impl PathWalker {
    fn extract_path(&mut self) {
        if self.recursive {
            for path in self.path.iter() {
                let mut dir: Vec<_> = read_dir(path).unwrap().map(|r| r.unwrap()).collect();
                dir.sort_by_key(|e| e.path());
                for entry in dir.iter() {
                    if is_dir(entry.path()).unwrap() {
                        self.path.push(entry.path());
                    } else {
                        self.files.push(entry.path());
                    }
                }
                self.path.remove(0);
                break;
            }
        }
    }

    pub fn new(p: PathBuf, recursive: bool) -> Self {
        let mut path = Vec::new();
        let mut files = Vec::new();
        if is_dir(&p).unwrap() {
            path.push(p);
        } else {
            files.push(p);
        }
        let mut walker = PathWalker {
            path,
            files,
            recursive: true,
        };
        walker.extract_path();
        walker.recursive = recursive;
        walker
    }
}

fn fs_walk_path<P: AsRef<Path>>(path: P, recursive: bool, callback: &impl Fn(&Path) -> bool) -> io::Result<bool> {
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
        Ok(callback(path.as_ref()))
    }
}

pub fn walk_path<P: AsRef<Path>>(path: P, recursive: bool, callback: impl Fn(&Path) -> bool) -> io::Result<()> {
    let _ = fs_walk_path(path, recursive, &callback)?;
    Ok(())
}

pub fn is_dir<P: AsRef<Path>>(path: P) -> io::Result<bool> {
    let meta = metadata(path.as_ref())?;
    Ok(meta.is_dir())
}