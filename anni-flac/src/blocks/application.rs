use std::io::{Read, Write};
use byteorder::{ReadBytesExt, BigEndian, WriteBytesExt};
use crate::prelude::*;
use crate::utils::*;
use std::fmt;

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

#[cfg(feature = "async")]
#[async_trait::async_trait]
impl AsyncDecode for BlockApplication {
    async fn from_async_reader<R>(reader: &mut R) -> Result<Self>
    where R: AsyncRead + Unpin + Send
    {
        Ok(BlockApplication {
            application_id: reader.read_u32().await?,
            data: take_to_end_async(reader).await?,
        })
    }
}

impl Encode for BlockApplication {
    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_u32::<BigEndian>(self.application_id)?;
        writer.write_all(&self.data)?;
        Ok(())
    }
}

impl fmt::Debug for BlockApplication {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut prefix = "".to_owned();
        if let Some(width) = f.width() {
            prefix = " ".repeat(width);
        }
        writeln!(f, "{prefix}application ID: {:x}", self.application_id, prefix = prefix)?;
        writeln!(f, "{prefix}data contents:", prefix = prefix)?;
        // TODO: hexdump
        writeln!(f, "{prefix}<TODO>", prefix = prefix)
    }
}