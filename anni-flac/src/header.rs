use std::io::{Read, Write, Seek, SeekFrom};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use crate::prelude::{Decode, Result, Encode};
use crate::blocks::*;
use crate::utils::skip;
use crate::error::FlacError;
use std::fmt;
use std::path::{Path, PathBuf};
use std::fs::{File, OpenOptions};

pub struct FlacHeader {
    pub blocks: Vec<MetadataBlock>,
    input_file: Option<PathBuf>,
    frame_offset: usize,
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
        let mut frame_offset = 4 + 4 + 34;
        while !is_last {
            let block = MetadataBlock::from_reader(reader)?;
            frame_offset += 4 + block.length;
            is_last = block.is_last;
            blocks.push(block);
        }
        Ok(FlacHeader { blocks, input_file: None, frame_offset })
    }

    pub fn from_file<P: AsRef<Path>>(filename: P) -> Result<FlacHeader> {
        let mut file = File::open(filename.as_ref())?;
        let mut header = Self::parse(&mut file)?;
        header.input_file = Some(filename.as_ref().to_path_buf());
        Ok(header)
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

    fn block_of_mut(&mut self, id: u8) -> Option<&mut MetadataBlock> {
        for block in self.blocks.iter_mut() {
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

    /// Get a mutable comments blocks for edit
    ///
    /// If VorbisComment block does not exist, a new block would be appended to header
    /// `is_last` would not be updated, please call `save` after header modify.
    pub fn comments_mut(&mut self) -> &mut BlockVorbisComment {
        let is_none = self.block_of_mut(4).is_none();
        if is_none {
            let comment = BlockVorbisComment { vendor_string: format!("anni-flac v{}", env!("CARGO_PKG_VERSION")), comments: vec![] };
            self.blocks.push(MetadataBlock::new(MetadataBlockData::Comment(comment)));
        }
        self.block_of_mut(4).map(|b| match &mut b.data {
            MetadataBlockData::Comment(c) => c,
            _ => unreachable!(),
        }).unwrap()
    }

    fn frame_offset_now(&self) -> usize {
        let mut frame_offset_now = 4;
        for block in self.blocks.iter() {
            frame_offset_now += 4 + block.data.len(); // block header + data
        }
        frame_offset_now
    }

    pub fn save<P: AsRef<Path>>(&mut self, output: Option<P>) -> Result<()> {
        let input_path = self.input_file.as_deref().expect("No input path provided!").to_path_buf();
        let output_path = match output {
            Some(p) => p.as_ref().to_path_buf(),
            None => input_path.clone(),
        };

        self.format();
        if input_path != output_path {
            // save to another file
            let mut file = File::create(output_path)?;

            // write magic number
            file.write_all(b"fLaC")?;
            // write header blocks
            for block in self.blocks.iter() {
                block.write_to(&mut file)?;
            }
            // write frames
            let mut file_input = File::open(input_path)?;
            file_input.seek(SeekFrom::Start(self.frame_offset as u64))?;
            std::io::copy(&mut file_input, &mut file)?;
        } else {
            // recalculate frame offset after header modify
            let frame_offset_now = self.frame_offset_now();

            let need_new_file = frame_offset_now > self.frame_offset || {
                // if header is smaller than / the same size as previous header
                // means we do not need more space
                // just need to write all data to the header
                let space_to_add = self.frame_offset - frame_offset_now;

                // try to get last block for padding
                let last = self.blocks.last_mut().unwrap();
                if let MetadataBlockData::Padding(size) = &mut last.data {
                    // padding block exists, modify padding size directly
                    last.length += space_to_add;
                    *size = last.length;
                    false
                } else if space_to_add >= 4 {
                    // padding block does not exist, add a new padding block
                    let space_to_add = space_to_add - 4;
                    last.is_last = false;
                    self.blocks.push(MetadataBlock {
                        is_last: true,
                        length: space_to_add,
                        data: MetadataBlockData::Padding(space_to_add),
                    });
                    false
                } else {
                    // a new padding block needs at least 4 bytes
                    // so if the space left is less than 4 bytes
                    // padding block can not be created
                    // we handle this situation as frame_offset_now > frame_offset_old
                    true
                }
            };
            if need_new_file {
                // write to filename.anni
                let output_new_path = output_path.with_extension("anni");
                self.save(Some(output_new_path.as_path()))?;

                let original_backup_path = output_path.with_extension("anni.bak");
                // move original file to filename.anni.bak
                std::fs::rename(&output_path, &original_backup_path)?;
                // move new file to original file
                std::fs::rename(output_new_path, output_path)?;
                // remove backup of original file
                std::fs::remove_file(original_backup_path)?;
            } else {
                // write back to input directly
                // so we only need to write header blocks to override the original header
                let mut file = OpenOptions::new().write(true).open(input_path)?;
                // skip magic number b"fLaC"
                file.seek(SeekFrom::Start(4))?;
                // write header blocks
                for block in self.blocks.iter() {
                    block.write_to(&mut file)?;
                }
            }
        }
        Ok(())
    }

    // TODO: make this method private
    pub fn format(&mut self) {
        // recalculate frame offset after header modify
        let frame_offset_now = self.frame_offset_now();

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
        if let Some(mut padding_block_size) = padding_size {
            let need_padding = frame_offset_now != self.frame_offset && if frame_offset_now > self.frame_offset {
                // need more space
                let needed = frame_offset_now - self.frame_offset;
                if needed <= padding_block_size {
                    // have enough space
                    padding_block_size -= frame_offset_now - self.frame_offset;
                    true
                } else if needed == padding_block_size + 4 {
                    // space needed == padding size + padding header size
                    padding_block_size = 0;
                    false
                } else {
                    // padding space not enough
                    padding_block_size = 8192;
                    true
                }
            } else {
                // expand padding space
                let expanded = self.frame_offset - frame_offset_now;
                padding_block_size += expanded;
                true
            };

            if need_padding {
                self.blocks.push(MetadataBlock {
                    is_last: true,
                    length: padding_block_size,
                    data: MetadataBlockData::Padding(padding_block_size),
                })
            }
        }

        // fix is_last identifier
        for block in self.blocks.iter_mut() {
            block.is_last = false;
        }
        self.blocks.last_mut().unwrap().is_last = true;
    }
}

pub struct MetadataBlock {
    // TODO: remove is_last flag, it's useless in a sequenced vector
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
        writer.write_u24::<BigEndian>(self.data.len() as u32)?;
        match &self.data {
            MetadataBlockData::StreamInfo(s) => s.write_to(writer)?,
            MetadataBlockData::Padding(p) => writer.write_all(&vec![0; *p])?, // FIXME: Why does writing zero needs to allocate memory?!
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
    pub fn new(data: MetadataBlockData) -> Self {
        MetadataBlock {
            is_last: false,
            length: 0,
            data,
        }
    }

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