use crate::common::{Decode, DecodeSized};
use std::io::Read;
use byteorder::{BigEndian, ReadBytesExt};
use crate::prelude::Result;
use crate::blocks::*;

pub struct FlacHeader {
    pub stream_info: (BlockStreamInfo, bool),
    pub blocks: Vec<MetadataBlock>,
}

pub struct MetadataBlock {
    pub is_last: bool,
    pub block_type: u8,
    pub length: usize,
    pub(crate) data: MetadataBlockData,
}

impl Decode for MetadataBlock {
    fn from_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let first_byte = reader.read_u8()?;
        let block_type = first_byte & 0b01111111;
        let length = reader.read_u24::<BigEndian>()? as usize;
        Ok(MetadataBlock {
            is_last: first_byte & 0b10000000 > 0,
            block_type,
            length,
            data: match block_type {
                0 => MetadataBlockData::StreamInfo(BlockStreamInfo::from_reader(&mut reader.take(length as u64))?),
                1 => MetadataBlockData::Padding(length),
                2 => MetadataBlockData::Application(BlockApplication::from_reader_sized(&mut reader.take(length as u64), length)?),
                3 => MetadataBlockData::SeekTable(BlockSeekTable::from_reader_sized(&mut reader.take(length as u64), length)?),
                4 => MetadataBlockData::Comment(BlockVorbisComment::from_reader(&mut reader.take(length as u64))?),
                5 => MetadataBlockData::CueSheet(BlockCueSheet::from_reader(&mut reader.take(length as u64))?),
                6 => MetadataBlockData::Picture(BlockPicture::from_reader(&mut reader.take(length as u64))?),
                _ => MetadataBlockData::Reserved((block_type, crate::common::take(reader, length)?)),
            },
        })
    }
}

pub enum MetadataBlockData {
    StreamInfo(BlockStreamInfo),
    Padding(usize),
    Application(BlockApplication),
    SeekTable(BlockSeekTable),
    Comment(BlockVorbisComment),
    CueSheet(BlockCueSheet),
    Picture(BlockPicture),
    Reserved((u8, Vec<u8>)),
}

