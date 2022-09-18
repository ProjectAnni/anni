use crate::workspace::target::WorkspaceTarget;
use std::path::{Path, PathBuf};

pub struct WorkspaceFileTarget(PathBuf);

impl WorkspaceFileTarget {
    pub fn new<P>(path: P) -> Self
    where
        P: Into<PathBuf>,
    {
        Self(path.into())
    }
}

impl WorkspaceTarget for WorkspaceFileTarget {
    fn mkdir<P>(&self, path: P) -> anyhow::Result<()>
    where
        P: AsRef<Path>,
    {
        Ok(std::fs::create_dir_all(self.0.join(path))?)
    }

    fn copy<P>(&self, src: P, dst: P) -> anyhow::Result<()>
    where
        P: AsRef<Path>,
    {
        std::fs::copy(src, self.0.join(dst))?;
        Ok(())
    }
}
