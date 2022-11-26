use crate::workspace::target::WorkspaceTarget;
use std::path::{Path, PathBuf};

pub struct WorkspaceFileTarget(pub PathBuf);

impl WorkspaceTarget for WorkspaceFileTarget {
    async fn mkdir<P>(&self, path: P) -> std::io::Result<()>
    where
        P: AsRef<Path>,
    {
        tokio::fs::create_dir_all(self.0.join(path)).await
    }

    async fn copy<P>(&self, src: P, dst: P) -> std::io::Result<()>
    where
        P: AsRef<Path>,
    {
        tokio::fs::copy(src, self.0.join(dst)).await.map(|_| ())
    }
}
