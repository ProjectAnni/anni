use std::io::{Result, Write};
use byteorder::{WriteBytesExt, LittleEndian, BigEndian};

pub fn btoken_w<W: Write>(writer: &mut W, tag: &[u8]) -> Result<()> {
    writer.write_all(tag)
}

#[inline]
pub fn u32_le_w<W: Write>(writer: &mut W, n: u32) -> Result<()> {
    writer.write_u32::<LittleEndian>(n)?;
    Ok(())
}

#[inline]
pub fn u32_be_w<W: Write>(writer: &mut W, n: u32) -> Result<()> {
    writer.write_u32::<BigEndian>(n)?;
    Ok(())
}

#[inline]
pub fn u24_le_w<W: Write>(writer: &mut W, n: u32) -> Result<()> {
    writer.write_u24::<LittleEndian>(n)?;
    Ok(())
}

#[inline]
pub fn u24_be_w<W: Write>(writer: &mut W, n: u32) -> Result<()> {
    writer.write_u24::<BigEndian>(n)?;
    Ok(())
}

#[inline]
pub fn u16_le_w<W: Write>(writer: &mut W, n: u16) -> Result<()> {
    writer.write_u16::<LittleEndian>(n)?;
    Ok(())
}

#[inline]
pub fn u16_be_w<W: Write>(writer: &mut W, n: u16) -> Result<()> {
    writer.write_u16::<BigEndian>(n)?;
    Ok(())
}
