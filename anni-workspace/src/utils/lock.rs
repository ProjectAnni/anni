use std::path::{Path, PathBuf};

use crate::WorkspaceError;

pub(crate) struct WorkspaceAlbumLock {
    lock_path: PathBuf,
}

impl WorkspaceAlbumLock {
    pub fn new<P>(album_path: P) -> Result<Self, WorkspaceError>
    where
        P: AsRef<Path>,
    {
        let lock_path = album_path.as_ref().join(".album.lock");
        if lock_path.exists() {
            return Err(WorkspaceError::AlbumLocked(
                album_path.as_ref().to_path_buf(),
            ));
        }

        Ok(Self { lock_path })
    }

    pub fn lock(&self) -> std::io::Result<()> {
        std::fs::File::create(&self.lock_path)?;
        Ok(())
    }
}

impl Drop for WorkspaceAlbumLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.lock_path);
    }
}
