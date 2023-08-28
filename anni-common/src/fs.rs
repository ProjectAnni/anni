use crate::decode::raw_to_string;
use log::debug;
use path_absolutize::*;
use std::ffi::OsString;
pub use std::fs::*;
use std::path::{Path, PathBuf};
use std::{fs, io};

pub struct PathWalker {
    path: Vec<PathBuf>,
    files: Vec<PathBuf>,
    recursive: bool,
    // whether to treat symlink file as regular file
    allow_symlink_file: bool,
    ignores: Vec<OsString>,
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
        if self.recursive && !self.path.is_empty() {
            let path = self.path.get(0).unwrap();
            let mut dir: Vec<_> = read_dir(path).unwrap().map(|r| r.unwrap()).collect();
            dir.sort_by_key(|e| e.path());
            for entry in dir.iter() {
                let metadata = entry.metadata().unwrap();
                if self.ignores.contains(&entry.file_name()) {
                    continue;
                }

                if metadata.is_dir() {
                    self.path.push(entry.path());
                } else if metadata.is_file() {
                    self.files.push(entry.path());
                } else {
                    // symlink
                    if self.allow_symlink_file {
                        // if it's a file, add it to files
                        if fs::metadata(entry.path()).unwrap().is_file() {
                            self.files.push(entry.path());
                        }
                    }
                }
            }
            self.path.remove(0);
        }
    }

    pub fn new<P: AsRef<Path>>(
        p: P,
        recursive: bool,
        allow_symlink_file: bool,
        ignores: Vec<String>,
    ) -> Self {
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
            allow_symlink_file,
            ignores: ignores.into_iter().map(|s| s.into()).collect(),
        };
        walker.extract_path();
        walker.recursive = recursive;
        walker
    }

    pub fn with_extensions(extensions: Box<[&str]>) -> Box<dyn Fn(&PathBuf) -> bool + '_> {
        Box::new(move |file: &PathBuf| match file.extension() {
            None => false,
            Some(ext) => extensions.contains(&ext.to_str().unwrap()),
        })
    }
}

fn fs_walk_path<P: AsRef<Path>>(
    path: P,
    recursive: bool,
    callback: &impl Fn(&Path) -> bool,
) -> io::Result<bool> {
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

pub fn walk_path<P: AsRef<Path>>(
    path: P,
    recursive: bool,
    callback: impl Fn(&Path) -> bool,
) -> io::Result<()> {
    let _ = fs_walk_path(path, recursive, &callback)?;
    Ok(())
}

pub fn is_dir<P: AsRef<Path>>(path: P) -> io::Result<bool> {
    let meta = metadata(path.as_ref())?;
    Ok(meta.is_dir())
}

pub fn get_ext_files<P: AsRef<Path>, T: AsRef<str>>(
    dir: P,
    ext: T,
    recursive: bool,
) -> io::Result<Vec<PathBuf>> {
    let mut result = Vec::new();
    if is_dir(dir.as_ref())? {
        for file in PathWalker::new(dir.as_ref(), recursive, true, Default::default()) {
            let file_ext = file
                .extension()
                .unwrap_or_default()
                .to_str()
                .unwrap_or_default();
            if file_ext == ext.as_ref() {
                result.push(file);
            }
        }
    }
    Ok(result)
}

pub fn get_ext_file<P: AsRef<Path>, T: AsRef<str>>(
    dir: P,
    ext: T,
    recursive: bool,
) -> io::Result<Option<PathBuf>> {
    if is_dir(dir.as_ref())? {
        for file in PathWalker::new(dir.as_ref(), recursive, true, Default::default()) {
            let file_ext = file
                .extension()
                .unwrap_or_default()
                .to_str()
                .unwrap_or_default();
            if file_ext == ext.as_ref() {
                return Ok(Some(file));
            }
        }
    }
    Ok(None)
}

pub fn get_subdirectories<P: AsRef<Path>>(dir: P) -> io::Result<Vec<PathBuf>> {
    let mut ret = Vec::new();
    let mut dir: Vec<_> = read_dir(dir.as_ref())?.map(|r| r.unwrap()).collect();
    dir.sort_by_key(|e| e.path());
    for dir in dir.iter() {
        let dir_type = dir.file_type()?;
        if dir_type.is_dir() {
            ret.push(dir.path());
        }
    }
    Ok(ret)
}

pub fn read_to_string<P: AsRef<Path>>(input: P) -> io::Result<String> {
    log::trace!("Reading file to string: {:?}", input.as_ref());
    let r = read(input)?;
    Ok(raw_to_string(&r))
}

#[cfg(feature = "trash")]
pub fn remove_file<P: AsRef<Path>>(input: P, trashcan: bool) -> io::Result<()> {
    if trashcan {
        trash::delete(input.as_ref()).map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    } else {
        fs::remove_file(input)
    }
}

#[cfg(feature = "trash")]
pub fn remove_dir_all<P: AsRef<Path>>(path: P, trashcan: bool) -> io::Result<()> {
    if trashcan {
        trash::delete(path).map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    } else {
        fs::remove_dir_all(path)
    }
}

/// Create symbolic link at `to` pointing to `from`
pub fn symlink_file<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> io::Result<()> {
    let link = path_diff(from, to.as_ref().parent().unwrap())?;
    #[cfg(unix)]
    return std::os::unix::fs::symlink(link, to);
    #[cfg(windows)]
    return std::os::windows::fs::symlink_file(link, to);
}

/// Create symbolic link at `to` pointing to `from`
pub fn symlink_dir<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> io::Result<()> {
    let link = path_diff(from, to.as_ref().parent().unwrap())?;
    #[cfg(unix)]
    return std::os::unix::fs::symlink(link, to);
    #[cfg(windows)]
    return std::os::windows::fs::symlink_dir(link, to);
}

pub fn path_diff<P: AsRef<Path>, Q: AsRef<Path>>(path: P, base: Q) -> io::Result<PathBuf> {
    Ok(pathdiff::diff_paths(path.as_ref().absolutize()?, base.as_ref().absolutize()?).unwrap())
}

pub fn copy_dir<P1, P2>(from: P1, to: P2) -> io::Result<()>
where
    P1: AsRef<Path>,
    P2: AsRef<Path>,
{
    create_dir(to.as_ref())?;

    for entry in read_dir(from)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let target = to.as_ref().join(entry.file_name());
        if file_type.is_file() {
            copy(entry.path(), target)?;
        } else if file_type.is_dir() {
            copy_dir(entry.path(), target)?;
        }
    }

    Ok(())
}

/// Move a directory from one location to another.
///
/// This method uses [rename] at first. If [rename] fails with [io::ErrorKind::CrossesDevices],
/// it will fallback to copying the directory and then removing the source directory.
pub fn move_dir<P1, P2>(from: P1, to: P2) -> io::Result<()>
where
    P1: AsRef<Path>,
    P2: AsRef<Path>,
{
    // check whether [from] is directory
    if !is_dir(from.as_ref())? {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} is not a directory", from.as_ref().display()),
        ));
    }

    match rename(from.as_ref(), to.as_ref()) {
        Err(e) if is_cross_device_error(&e) => {
            debug!("Failed to rename across filesystems. Copying instead.");

            copy_dir(from.as_ref(), to.as_ref())?;
            debug!("Copying done. Removing source directory.");

            fs::remove_dir_all(from.as_ref())?;
            debug!("Source directory removed.");
        }
        _ => {}
    };

    Ok(())
}

fn is_cross_device_error(error: &io::Error) -> bool {
    let code = error.raw_os_error();
    #[cfg(windows)]
    {
        code == Some(17)
    }
    #[cfg(unix)]
    {
        code == Some(18)
    }
    #[cfg(all(not(windows), not(unix)))]
    {
        // unsupported platform
        false
    }
}
