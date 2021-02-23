use std::io::{Read, BufReader};
use std::io;
use thiserror::Error;
use byteorder::{ReadBytesExt};
use std::string::FromUtf8Error;

pub struct FlacDecoder<R> {
    inner: BufReader<R>,
}

impl<R: Read> FlacDecoder<R> {
    pub fn new(r: R) -> Result<FlacDecoder<R>, FlacError> {
        let mut ret = FlacDecoder { inner: BufReader::new(r) };
        if ret.inner.read_u8()? != b'f' ||
            ret.inner.read_u8()? != b'L' ||
            ret.inner.read_u8()? != b'a' ||
            ret.inner.read_u8()? != b'C' {
            return Err(FlacError::InvalidMagicNumber);
        }
        Ok(ret)
    }
}

#[derive(Error, Debug)]
pub enum FlacError {
    #[error("invalid magic number")]
    InvalidMagicNumber,
    #[error("invalid block type 0xff")]
    InvalidBlockType,
    #[error("invalid seektable size")]
    InvalidSeekTableSize,
    #[error(transparent)]
    InvalidString(#[from] FromUtf8Error),
    #[error(transparent)]
    IO(#[from] io::Error),
}