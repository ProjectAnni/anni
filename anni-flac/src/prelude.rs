use std::io::Read;
use byteorder::ReadBytesExt;
use crate::error::FlacError;
use crate::header::{FlacHeader, MetadataBlock};

pub type Result<I> = std::result::Result<I, crate::error::FlacError>;

pub trait Decode: Sized {
    fn from_reader<R: Read>(reader: &mut R) -> Result<Self>;
}

pub fn decode_header<R: Read>(reader: &mut R, skip_magic_number: bool) -> Result<FlacHeader> {
    if !skip_magic_number {
        if reader.read_u8()? != b'f' ||
            reader.read_u8()? != b'L' ||
            reader.read_u8()? != b'a' ||
            reader.read_u8()? != b'C' {
            return Err(FlacError::InvalidMagicNumber);
        }
    }
    let stream_info = MetadataBlock::from_reader(reader)?;
    let mut is_last = stream_info.is_last;
    let mut blocks = vec![stream_info];
    while !is_last {
        let block = MetadataBlock::from_reader(reader)?;
        is_last = block.is_last;
        blocks.push(block);
    }
    Ok(FlacHeader { blocks })
}
