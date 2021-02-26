use std::io::Read;

pub type Result<I> = std::result::Result<I, crate::error::FlacError>;

pub trait Decode: Sized {
    fn from_reader<R: Read>(reader: &mut R) -> Result<Self>;
}
