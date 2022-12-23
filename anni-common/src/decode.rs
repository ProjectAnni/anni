use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use std::io;
use std::io::Read;
use std::string::FromUtf8Error;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error(transparent)]
    IOError(#[from] io::Error),
    #[error(transparent)]
    FromUtf8Error(#[from] FromUtf8Error),
    #[error("invalid token, expected {expected:?}, got {got:?}")]
    InvalidTokenError { expected: Vec<u8>, got: Vec<u8> },
}

type DecodeResult<T> = std::result::Result<T, DecodeError>;

pub fn take<R: Read>(reader: &mut R, len: usize) -> DecodeResult<Vec<u8>> {
    let (r, _) = take_sized(reader, len)?;
    Ok(r)
}

pub fn take_sized<R: Read>(reader: &mut R, len: usize) -> std::io::Result<(Vec<u8>, u64)> {
    let mut r = Vec::with_capacity(len);
    let got = std::io::copy(&mut reader.take(len as u64), &mut r)?;
    Ok((r, got))
}

pub fn take_to_end<R: Read>(reader: &mut R) -> DecodeResult<Vec<u8>> {
    let mut r = Vec::new();
    reader.read_to_end(&mut r)?;
    Ok(r)
}

#[inline]
pub fn take_string<R: Read>(reader: &mut R, len: usize) -> DecodeResult<String> {
    Ok(String::from_utf8(take(reader, len)?)?)
}

#[inline]
pub fn skip<R: Read>(reader: &mut R, len: usize) -> DecodeResult<u64> {
    Ok(std::io::copy(
        &mut reader.take(len as u64),
        &mut std::io::sink(),
    )?)
}

pub fn token<R: Read>(reader: &mut R, token: &[u8]) -> DecodeResult<()> {
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
pub fn u8<R: Read>(reader: &mut R) -> DecodeResult<u8> {
    Ok(reader.read_u8()?)
}

#[inline]
pub fn u32_le<R: Read>(reader: &mut R) -> DecodeResult<u32> {
    Ok(reader.read_u32::<LittleEndian>()?)
}

#[inline]
pub fn u32_be<R: Read>(reader: &mut R) -> DecodeResult<u32> {
    Ok(reader.read_u32::<BigEndian>()?)
}

#[inline]
pub fn u16_le<R: Read>(reader: &mut R) -> DecodeResult<u16> {
    Ok(reader.read_u16::<LittleEndian>()?)
}

#[inline]
pub fn u16_be<R: Read>(reader: &mut R) -> DecodeResult<u16> {
    Ok(reader.read_u16::<BigEndian>()?)
}

#[inline]
pub fn u24_le<R: Read>(reader: &mut R) -> DecodeResult<u32> {
    Ok(reader.read_u24::<LittleEndian>()?)
}

#[inline]
pub fn u24_be<R: Read>(reader: &mut R) -> DecodeResult<u32> {
    Ok(reader.read_u24::<BigEndian>()?)
}

pub fn raw_to_string(input: &[u8]) -> String {
    let mut detector = chardetng::EncodingDetector::new();
    detector.feed(input, true);
    let (result, encoding, _) = detector.guess(None, true).decode(input);
    log::trace!("Encoding detected: {}", encoding.name());
    result.into()
}

// https://github.com/serde-rs/serde/issues/1425#issuecomment-439729881
pub fn non_empty_str<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    use serde::Deserialize;
    let o: Option<String> = Option::deserialize(d)?;
    Ok(o.filter(|s| !s.is_empty()))
}
