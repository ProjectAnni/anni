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

    pub fn new<P: AsRef<Path>>(p: P, recursive: bool) -> Self {
        let mut path = Vec::new();
        let mut files = Vec::new();
        if is_dir(&p).unwrap() {
            path.push(p.as_ref().to_owned());
        } else {
            files.push(p.as_ref().to_owned());
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

    pub fn with_extensions(exts: Box<[&str]>) -> Box<dyn Fn(&PathBuf) -> bool + '_>
    {
        Box::new(move |file: &PathBuf| {
            match file.extension() {
                None => false,
                Some(ext) => {
                    exts.contains(&ext.to_str().unwrap())
                }
            }
        })
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

pub fn get_ext_files<P: AsRef<Path>, T: AsRef<str>>(dir: P, ext: T, recursive: bool) -> io::Result<Option<Vec<PathBuf>>> {
    let mut result = Vec::new();
    if is_dir(dir.as_ref())? {
        for file in PathWalker::new(dir.as_ref(), recursive) {
            let file_ext = file.extension().unwrap_or("".as_ref()).to_str().unwrap_or("");
            if file_ext == ext.as_ref() {
                result.push(file);
            }
        }
    }
    if result.is_empty() {
        Ok(None)
    } else {
        Ok(Some(result))
    }
}

pub fn get_ext_file<P: AsRef<Path>, T: AsRef<str>>(dir: P, ext: T, recursive: bool) -> io::Result<Option<PathBuf>> {
    if is_dir(dir.as_ref())? {
        for file in PathWalker::new(dir.as_ref(), recursive) {
            let file_ext = file.extension().unwrap_or("".as_ref()).to_str().unwrap_or("");
            if file_ext == ext.as_ref() {
                return Ok(Some(file));
            }
        }
    }
    Ok(None)
}

pub fn get_subdirectories<P: AsRef<Path>>(dir: P) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut ret = Vec::new();
    let mut dir: Vec<_> = read_dir(dir.as_ref())?.map(|r| r.unwrap()).collect();
    dir.sort_by_key(|e| e.path());
    for dir in dir.iter() {
        if is_dir(dir.path())? {
            ret.push(dir.path());
        }
    }
    Ok(ret)
}