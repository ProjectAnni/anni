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
    InvalidTokenError {
        expected: Vec<u8>,
        got: Vec<u8>,
    },
}

pub fn take<R: Read>(reader: &mut R, len: usize) -> Result<Vec<u8>, DecodeError> {
    let mut r = Vec::with_capacity(len);
    std::io::copy(&mut reader.take(len as u64), &mut r)?;
    Ok(r)
}

pub fn take_to_end<R: Read>(reader: &mut R) -> Result<Vec<u8>, DecodeError> {
    let mut r = Vec::new();
    reader.read_to_end(&mut r)?;
    Ok(r)
}

pub fn take_string<R: Read>(reader: &mut R, len: usize) -> Result<String, DecodeError> {
    let r = take(reader, len)?;
    Ok(String::from_utf8(r)?)
}

pub fn skip<R: Read>(reader: &mut R, len: usize) -> Result<u64, DecodeError> {
    Ok(std::io::copy(&mut reader.take(len as u64), &mut std::io::sink())?)
}

pub fn token<R: Read>(reader: &mut R, token: &[u8]) -> Result<(), DecodeError> {
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
