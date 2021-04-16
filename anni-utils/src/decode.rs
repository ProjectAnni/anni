use std::io;
use std::io::Read;
use std::string::FromUtf8Error;
use thiserror::Error;
use byteorder::{ReadBytesExt, BigEndian, LittleEndian};

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error(transparent)]
    IOError(#[from] io::Error),
    #[error(transparent)]
    FromUtf8Error(#[from] FromUtf8Error),
    #[error("invalid token, expected {expected:?}, got {got:?}")]
    InvalidTokenError {
        expected: Vec<u8>,
        got: Vec<u8>,
    },
}

type Result<T> = std::result::Result<T, DecodeError>;

pub fn take<R: Read>(reader: &mut R, len: usize) -> Result<Vec<u8>> {
    let (r, _) = take_sized(reader, len)?;
    Ok(r)
}

pub fn take_sized<R: Read>(reader: &mut R, len: usize) -> std::io::Result<(Vec<u8>, u64)> {
    let mut r = Vec::with_capacity(len);
    let got = std::io::copy(&mut reader.take(len as u64), &mut r)?;
    Ok((r, got))
}

pub fn take_to_end<R: Read>(reader: &mut R) -> Result<Vec<u8>> {
    let mut r = Vec::new();
    reader.read_to_end(&mut r)?;
    Ok(r)
}

#[inline]
pub fn take_string<R: Read>(reader: &mut R, len: usize) -> Result<String> {
    Ok(String::from_utf8(take(reader, len)?)?)
}

#[inline]
pub fn skip<R: Read>(reader: &mut R, len: usize) -> Result<u64> {
    Ok(std::io::copy(&mut reader.take(len as u64), &mut std::io::sink())?)
}

pub fn token<R: Read>(reader: &mut R, token: &[u8]) -> Result<()> {
    let got = take(reader, token.len())?;
    if got[..] == token[..] {
        Ok(())
    } else {
        Err(DecodeError::InvalidTokenError {
            expected: token.to_owned(),
            got,
        })
    }
}

#[inline]
pub fn u8<R: Read>(reader: &mut R) -> Result<u8> {
    Ok(reader.read_u8()?)
}

#[inline]
pub fn u32_le<R: Read>(reader: &mut R) -> Result<u32> {
    Ok(reader.read_u32::<LittleEndian>()?)
}

#[inline]
pub fn u32_be<R: Read>(reader: &mut R) -> Result<u32> {
    Ok(reader.read_u32::<BigEndian>()?)
}

#[inline]
pub fn u16_le<R: Read>(reader: &mut R) -> Result<u16> {
    Ok(reader.read_u16::<LittleEndian>()?)
}

#[inline]
pub fn u16_be<R: Read>(reader: &mut R) -> Result<u16> {
    Ok(reader.read_u16::<BigEndian>()?)
}

#[inline]
pub fn u24_le<R: Read>(reader: &mut R) -> Result<u32> {
    Ok(reader.read_u24::<LittleEndian>()?)
}

#[inline]
pub fn u24_be<R: Read>(reader: &mut R) -> Result<u32> {
    Ok(reader.read_u24::<BigEndian>()?)
}

pub fn raw_to_string(input: &[u8]) -> String {
    let mut detector = chardetng::EncodingDetector::new();
    detector.feed(input, true);
    let (result, encoding, _) = detector.guess(None, true).decode(input);
    log::debug!("Encoding detected: {}", encoding.name());
    result.into()
}
