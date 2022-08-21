use crate::prelude::*;
use std::io::Read;

pub(crate) fn take<R: Read>(reader: &mut R, len: usize) -> std::io::Result<Vec<u8>> {
    let mut r = Vec::with_capacity(len);
    std::io::copy(&mut reader.take(len as u64), &mut r)?;
    Ok(r)
}

#[cfg(feature = "async")]
pub(crate) async fn take_async<R: AsyncRead + Unpin>(
    reader: &mut R,
    len: usize,
) -> std::io::Result<Vec<u8>> {
    let mut r = Vec::with_capacity(len);
    tokio::io::copy(&mut reader.take(len as u64), &mut r).await?;
    Ok(r)
}

pub(crate) fn take_to_end<R: Read>(reader: &mut R) -> std::io::Result<Vec<u8>> {
    let mut r = Vec::new();
    reader.read_to_end(&mut r)?;
    Ok(r)
}

#[cfg(feature = "async")]
pub(crate) async fn take_to_end_async<R: AsyncRead + Unpin>(
    reader: &mut R,
) -> std::io::Result<Vec<u8>> {
    let mut r = Vec::new();
    reader.read_to_end(&mut r).await?;
    Ok(r)
}

pub(crate) fn take_string<R: Read>(reader: &mut R, len: usize) -> Result<String> {
    let r = take(reader, len)?;
    Ok(String::from_utf8_lossy(&r).to_string())
}

#[cfg(feature = "async")]
pub(crate) async fn take_string_async<R: AsyncRead + Unpin>(
    reader: &mut R,
    len: usize,
) -> Result<String> {
    let r = take_async(reader, len).await?;
    Ok(String::from_utf8_lossy(&r).to_string())
}

pub(crate) fn skip<R: Read>(reader: &mut R, len: usize) -> std::io::Result<u64> {
    std::io::copy(&mut reader.take(len as u64), &mut std::io::sink())
}

#[cfg(feature = "async")]
pub(crate) async fn skip_async<R: AsyncRead + Unpin>(
    reader: &mut R,
    len: usize,
) -> std::io::Result<u64> {
    tokio::io::copy(&mut reader.take(len as u64), &mut tokio::io::sink()).await
}

#[cfg(feature = "async")]
pub(crate) async fn read_u24_async<R: AsyncRead + Unpin>(reader: &mut R) -> std::io::Result<u32> {
    use byteorder::ByteOrder;

    let mut buf = [0; 3];
    reader.read_exact(&mut buf).await?;
    Ok(byteorder::BigEndian::read_u24(&buf))
}
