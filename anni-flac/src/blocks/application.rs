use crate::{DecodeSized, Result};
use std::io::Read;
use byteorder::{ReadBytesExt, BigEndian};

#[derive(Debug)]
pub struct BlockApplication {
    /// Registered application ID.
    /// (Visit the [registration page](https://xiph.org/flac/id.html) to register an ID with FLAC.)
    pub application_id: u32,
    /// Application data (n must be a multiple of 8)
    pub data: Vec<u8>,
}

impl DecodeSized for BlockApplication {
    fn from_reader_sized<R: Read>(reader: &mut R, size: usize) -> Result<Self> {
        Ok(BlockApplication {
            application_id: reader.read_u32::<BigEndian>()?,
            data: crate::common::take(reader, size - 4)?,
        })
    }
}
