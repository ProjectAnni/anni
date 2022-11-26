use std::path::Path;

mod drive;
mod file;

pub trait WorkspaceTarget {
    async fn mkdir<P>(&self, path: P) -> std::io::Result<()>
    where
        P: AsRef<Path>;

    async fn copy<P>(&self, src: P, dst: P) -> std::io::Result<()>
    where
        P: AsRef<Path>;
}
