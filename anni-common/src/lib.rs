use std::path::Path;

pub trait FromFile: Sized {
    fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, anyhow::Error>;
}
