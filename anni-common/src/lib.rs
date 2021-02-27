use std::path::Path;
use std::io::Read;

pub trait FromFile: Sized {
    fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, anyhow::Error>;
}

pub trait Decode: Sized {
    type Err;

    fn from_reader<R: Read>(reader: &mut R) -> Result<Self, Self::Err>;
}
