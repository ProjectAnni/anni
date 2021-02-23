use std::io::Read;
use crate::Result;

pub trait Decode: Sized {
    fn from_reader<R: Read>(reader: &mut R) -> Result<Self>;
}

pub trait DecodeSized: Sized {
    fn from_reader_sized<R: Read>(reader: &mut R, size: usize) -> Result<Self>;
}

pub(crate) fn take<R: Read>(reader: &mut R, len: usize) -> std::io::Result<Vec<u8>> {
    let mut r: Vec<u8> = Vec::with_capacity(len);
    reader.read_exact(&mut r)?;
    Ok(r)
}

pub(crate) fn take_string<R: Read>(reader: &mut R, len: usize) -> Result<String> {
    let vec = take(reader, len)?;
    Ok(String::from_utf8(vec)?)
}