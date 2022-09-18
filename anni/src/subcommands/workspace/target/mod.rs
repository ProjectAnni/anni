use std::path::Path;

mod drive;
mod file;

pub trait WorkspaceTarget {
    fn mkdir<P>(&self, path: P) -> anyhow::Result<()>
    where
        P: AsRef<Path>;

    fn copy<P>(&self, src: P, dst: P) -> anyhow::Result<()>
    where
        P: AsRef<Path>;
}
