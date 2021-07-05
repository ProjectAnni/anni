use std::io::{Read, Write};

pub type Result<I> = std::result::Result<I, crate::error::FlacError>;

pub trait Decode: Sized {
    fn from_reader<R: Read>(reader: &mut R) -> Result<Self>;
}

pub trait Encode: Sized {
    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()>;
}
