use std::io::Read;
use crate::prelude::Result;

pub(crate) fn take<R: Read>(reader: &mut R, len: usize) -> std::io::Result<Vec<u8>> {
    let mut r: Vec<u8> = Vec::with_capacity(len);
    reader.read_exact(&mut r)?;
    Ok(r)
}

pub(crate) fn take_to_end<R: Read>(reader: &mut R) -> std::io::Result<Vec<u8>> {
    let mut r: Vec<u8> = Vec::new();
    reader.read_to_end(&mut r)?;
    Ok(r)
}

pub(crate) fn take_string<R: Read>(reader: &mut R, len: usize) -> Result<String> {
    let vec = take(reader, len)?;
    Ok(String::from_utf8(vec)?)
}

pub(crate) fn skip<R: Read>(reader: &mut R, len: usize) -> std::io::Result<u64> {
    std::io::copy(&mut reader.take(len as u64), &mut std::io::sink())
}
