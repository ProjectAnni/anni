use std::io::{Read, Write};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use crate::prelude::{Decode, Result, Encode};
use crate::blocks::*;
use crate::utils::skip;
use std::fmt;
use crate::error::FlacError;
use std::path::Path;
use std::fs::File;

pub struct FlacHeader {
    pub blocks: Vec<MetadataBlock>,
}

impl FlacHeader {
    pub fn parse<R: Read>(reader: &mut R) -> Result<FlacHeader> {
        if reader.read_u8()? != b'f' ||
            reader.read_u8()? != b'L' ||
            reader.read_u8()? != b'a' ||
            reader.read_u8()? != b'C' {
            return Err(FlacError::InvalidMagicNumber);
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

    pub fn from_file<P: AsRef<Path>>(filename: P) -> Result<FlacHeader> {
        let mut file = File::open(filename)?;
        Self::parse(&mut file)
    }

    pub fn stream_info(&self) -> &BlockStreamInfo {
        let block = self.blocks.iter().nth(0).unwrap();
        match &block.data {
            MetadataBlockData::StreamInfo(i) => i,
            _ => panic!("First block is not stream info!"),
        }
    }

    fn block_of(&self, id: u8) -> Option<&MetadataBlock> {
        for block in self.blocks.iter() {
            if u8::from(&block.data) == id {
                return Some(block);
            }
        }
        None
    }

    pub fn comments(&self) -> Option<&BlockVorbisComment> {
        self.block_of(4).map(|b| match &b.data {
            MetadataBlockData::Comment(c) => c,
            _ => unreachable!(),
        })
    }

    pub fn format(&mut self) {
        // merge padding blocks
        let mut padding_size: Option<usize> = None;
        self.blocks.retain(|block| match &block.data {
            MetadataBlockData::Padding(size) => {
                // update padding block size
                padding_size = Some(padding_size.unwrap_or_default() + size);
                // remove all padding blocks
                false
            }
            // keep all other blocks
            _ => true,
        });

        // insert padding block if necessary
        if let Some(padding_block_size) = padding_size {
            self.blocks.push(MetadataBlock {
                is_last: true,
                length: padding_block_size,
                data: MetadataBlockData::Padding(padding_block_size),
            })
        }

        // fix is_last identifier
        for block in self.blocks.iter_mut() {
            block.is_last = false;
        }
        self.blocks.last_mut().unwrap().is_last = true;
    }
}

pub struct MetadataBlock {
    pub is_last: bool,
    pub length: usize,
    pub data: MetadataBlockData,
}

impl Decode for MetadataBlock {
    fn from_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let first_byte = reader.read_u8()?;
        let block_type = first_byte & 0b01111111;
        let length = reader.read_u24::<BigEndian>()? as usize;
        Ok(MetadataBlock {
            is_last: first_byte & 0b10000000 > 0,
            length,
            data: match block_type {
                0 => MetadataBlockData::StreamInfo(BlockStreamInfo::from_reader(&mut reader.take(length as u64))?),
                1 => MetadataBlockData::Padding(skip(reader, length)? as usize),
                2 => MetadataBlockData::Application(BlockApplication::from_reader(&mut reader.take(length as u64))?),
                3 => MetadataBlockData::SeekTable(BlockSeekTable::from_reader(&mut reader.take(length as u64))?),
                4 => MetadataBlockData::Comment(BlockVorbisComment::from_reader(&mut reader.take(length as u64))?),
                5 => MetadataBlockData::CueSheet(BlockCueSheet::from_reader(&mut reader.take(length as u64))?),
                6 => MetadataBlockData::Picture(BlockPicture::from_reader(&mut reader.take(length as u64))?),
                _ => MetadataBlockData::Reserved((block_type, crate::utils::take(reader, length)?)),
            },
        })
    }
}

impl Encode for MetadataBlock {
    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_u8((if self.is_last { 0b10000000 } else { 0 }) + u8::from(&self.data))?;
        writer.write_u24::<BigEndian>(self.length as u32)?;
        match &self.data {
            MetadataBlockData::StreamInfo(s) => s.write_to(writer)?,
            MetadataBlockData::Padding(p) => writer.write_all(&vec![0u8; *p])?, // FIXME: Why does writing zero needs to allocate memory?!
            MetadataBlockData::Application(a) => a.write_to(writer)?,
            MetadataBlockData::SeekTable(s) => s.write_to(writer)?,
            MetadataBlockData::Comment(c) => c.write_to(writer)?,
            MetadataBlockData::CueSheet(c) => c.write_to(writer)?,
            MetadataBlockData::Picture(p) => p.write_to(writer)?,
            MetadataBlockData::Reserved((_, data)) => writer.write_all(data)?,
        }
        Ok(())
    }
}

impl MetadataBlock {
    pub fn print(&self, i: usize) {
        let data = &self.data;
        println!("METADATA block #{}", i);
        println!("  type: {} ({})", u8::from(data), data.as_str());
        println!("  is last: {}", &self.is_last);
        println!("  length: {}", &self.length);
        println!("{:2?}", data);
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

impl From<&MetadataBlockData> for u8 {
    fn from(data: &MetadataBlockData) -> Self {
        match data {
            MetadataBlockData::StreamInfo(_) => 0,
            MetadataBlockData::Padding(_) => 1,
            MetadataBlockData::Application(_) => 2,
            MetadataBlockData::SeekTable(_) => 3,
            MetadataBlockData::Comment(_) => 4,
            MetadataBlockData::CueSheet(_) => 5,
            MetadataBlockData::Picture(_) => 6,
            MetadataBlockData::Reserved((t, _)) => *t,
        }
    }
}

impl MetadataBlockData {
    pub fn as_str(&self) -> &'static str {
        match self {
            MetadataBlockData::StreamInfo(_) => "STREAMINFO",
            MetadataBlockData::Padding(_) => "PADDING",
            MetadataBlockData::Application(_) => "APPLICATION",
            MetadataBlockData::SeekTable(_) => "SEEKTABLE",
            MetadataBlockData::Comment(_) => "VORBIS_COMMENT",
            MetadataBlockData::CueSheet(_) => "CUESHEET",
            MetadataBlockData::Picture(_) => "PICTURE",
            _ => "RESERVED",
        }
    }

    pub fn len(&self) -> usize {
        match self {
            MetadataBlockData::StreamInfo(_) => 34,
            MetadataBlockData::Padding(p) => *p,
            MetadataBlockData::Application(a) => a.data.len() + 4,
            MetadataBlockData::SeekTable(t) => t.seek_points.len() * 18,
            MetadataBlockData::Comment(c) => 8 + c.vendor_string.len() + c.comments.iter().map(|c| 4 + c.len()).sum::<usize>(),
            MetadataBlockData::CueSheet(c) => 396 + c.tracks.iter().map(|t| 36 + t.track_index.len() * 12).sum::<usize>(),
            MetadataBlockData::Picture(p) => 32 + p.mime_type.len() + p.description.len() + p.data.len(),
            MetadataBlockData::Reserved((_, arr)) => arr.len(),
        }
    }
}

impl fmt::Debug for MetadataBlockData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let prefix = if let Some(prefix) = f.width() {
            prefix
        } else {
            0
        };
        match self {
            MetadataBlockData::Padding(_) => Ok(()),
            MetadataBlockData::Reserved(_) => Ok(()),
            MetadataBlockData::StreamInfo(s) => write!(f, "{:prefix$?}", s, prefix = prefix),
            MetadataBlockData::Application(s) => write!(f, "{:prefix$?}", s, prefix = prefix),
            MetadataBlockData::SeekTable(s) => write!(f, "{:prefix$?}", s, prefix = prefix),
            MetadataBlockData::Comment(s) => write!(f, "{:prefix$?}", s, prefix = prefix),
            MetadataBlockData::CueSheet(s) => write!(f, "{:prefix$?}", s, prefix = prefix),
            MetadataBlockData::Picture(s) => write!(f, "{:prefix$?}", s, prefix = prefix),
        }
    }
}