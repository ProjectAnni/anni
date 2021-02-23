use std::io::Read;
use byteorder::{BigEndian, ReadBytesExt};
use crate::prelude::{Decode, Result};
use crate::blocks::*;
use crate::utils::skip;

pub struct FlacHeader {
    pub blocks: Vec<MetadataBlock>,
}

impl FlacHeader {
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

impl MetadataBlock {
    pub fn print(&self, i: usize) {
        let data = &self.data;
        println!("METADATA block #{}", i);
        println!("  type: {} ({})", u8::from(data), data.as_str());
        println!("  is last: {}", &self.is_last);
        println!("  length: {}", &self.length);
        match data {
            MetadataBlockData::StreamInfo(s) => {
                println!("  minimum blocksize: {} samples", s.min_block_size);
                println!("  maximum blocksize: {} samples", s.max_block_size);
                println!("  minimum framesize: {} bytes", s.min_frame_size);
                println!("  maximum framesize: {} bytes", s.max_frame_size);
                println!("  sample_rate: {} Hz", s.sample_rate);
                println!("  channels: {}", s.channels);
                println!("  bits-per-sample: {}", s.bits_per_sample);
                println!("  total samples: {}", s.total_samples);
                println!("  MD5 signature: {}", hex::encode(s.md5_signature));
            }
            MetadataBlockData::Application(s) => {
                println!("  application ID: {:x}", s.application_id);
                println!("  data contents:");
                // TODO: hexdump
                println!("  <TODO>");
            }
            MetadataBlockData::SeekTable(s) => {
                println!("  seek points: {}", s.seek_points.len());
                for (i, p) in s.seek_points.iter().enumerate() {
                    if p.is_placehoder() {
                        println!("    point {}: PLACEHOLDER", i);
                    } else {
                        println!("    point {}: sample_number={}, stream_offset={}, frame_samples={}", i, p.sample_number, p.stream_offset, p.frame_samples);
                    }
                }
            }
            MetadataBlockData::Comment(s) => {
                println!("  vendor string: {}", s.vendor_string);
                println!("  comments: {}", s.len());
                for (i, c) in s.comments.iter().enumerate() {
                    println!("    comment[{}]: {}={}", i, c.key_raw(), c.value());
                }
            }
            MetadataBlockData::CueSheet(s) => {
                println!("  media catalog number: {}", s.catalog_number);
                println!("  lead-in: {}", s.leadin_samples);
                println!("  is CD: {}", s.is_cd);
                println!("  number of tracks: {}", s.track_number);
                for (i, t) in s.tracks.iter().enumerate() {
                    println!("    track[{}]", i);
                    println!("      offset: {}", t.track_offset);
                    // TODO: https://github.com/xiph/flac/blob/ce6dd6b5732e319ef60716d9cc9af6a836a4011a/src/metaflac/operations.c#L627-L651
                }
            }
            MetadataBlockData::Picture(s) => {
                println!("  type: {} ({})", s.picture_type as u8, s.picture_type.as_str());
                println!("  MIME type: {}", s.mime_type);
                println!("  description: {}", s.description);
                println!("  width: {}", s.width);
                println!("  height: {}", s.height);
                println!("  depth: {}", s.depth);
                println!("  colors: {}{}", s.colors, if s.color_indexed() { "" } else { " (unindexed)" });
                println!("  data length: {}", s.data.len());
                println!("  data:");
                // TODO: hexdump
                println!("  <TODO>");
            }
            _ => {}
        }
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
}

