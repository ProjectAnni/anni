use std::io::Read;
use byteorder::{ReadBytesExt, BigEndian};
use crate::prelude::{Decode, Result};
use crate::utils::take_to_end;
use std::fmt;

#[derive(Debug)]
pub struct BlockApplication {
    /// Registered application ID.
    /// (Visit the [registration page](https://xiph.org/flac/id.html) to register an ID with FLAC.)
    pub application_id: u32,
    /// Application data (n must be a multiple of 8)
    pub data: Vec<u8>,
}

impl Decode for BlockApplication {
    fn from_reader<R: Read>(reader: &mut R) -> Result<Self> {
        Ok(BlockApplication {
            application_id: reader.read_u32::<BigEndian>()?,
            data: take_to_end(reader)?,
        })
    }
}

impl fmt::Display for BlockApplication {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "  application ID: {:x}", self.application_id)?;
        writeln!(f, "  data contents:")?;
        // TODO: hexdump
        writeln!(f, "  <TODO>")
    }
}