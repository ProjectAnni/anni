use crate::error::FlacError;
use crate::prelude::*;
use crate::utils::*;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::fmt;
use std::io::{Read, Write};

pub struct BlockSeekTable {
    pub seek_points: Vec<SeekPoint>,
}

/// Notes:
/// - For placeholder points, the second and third field values are undefined.
/// - Seek points within a table must be sorted in ascending order by sample number.
/// - Seek points within a table must be unique by sample number, with the exception of placeholder points.
/// - The previous two notes imply that there may be any number of placeholder points, but they must all occur at the end of the table.
#[derive(Debug)]
pub struct SeekPoint {
    // Sample number of first sample in the target frame, or 0xFFFFFFFFFFFFFFFF for a placeholder point.
    pub sample_number: u64,
    // Offset (in bytes) from the first byte of the first frame header to the first byte of the target frame's header.
    pub stream_offset: u64,
    // Number of samples in the target frame.
    pub frame_samples: u16,
}

impl SeekPoint {
    pub fn is_placeholder(&self) -> bool {
        self.sample_number == 0xFFFFFFFFFFFFFFFF
    }
}

impl Decode for BlockSeekTable {
    fn from_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let buf = take_to_end(reader)?;
        let size = buf.len();
        let mut reader = std::io::Cursor::new(buf);

        // The number of seek points is implied by the metadata header 'length' field, i.e. equal to length / 18.
        let points = size / 18;
        let remaining = size % 18;
        if remaining != 0 {
            return Err(FlacError::InvalidSeekTableSize);
        }

        let mut seek_points = Vec::with_capacity(points);
        for _ in 0..points {
            let sample_number = ReadBytesExt::read_u64::<BigEndian>(&mut reader)?;
            let stream_offset = ReadBytesExt::read_u64::<BigEndian>(&mut reader)?;
            let frame_samples = ReadBytesExt::read_u16::<BigEndian>(&mut reader)?;
            seek_points.push(SeekPoint {
                sample_number,
                stream_offset,
                frame_samples,
            });
        }

        Ok(BlockSeekTable { seek_points })
    }
}

#[cfg(feature = "async")]
#[async_trait::async_trait]
impl AsyncDecode for BlockSeekTable {
    async fn from_async_reader<R>(reader: &mut R) -> Result<Self>
    where
        R: AsyncRead + Unpin + Send,
    {
        let buf = take_to_end_async(reader).await?;
        let size = buf.len();
        let mut reader = std::io::Cursor::new(buf);

        // The number of seek points is implied by the metadata header 'length' field, i.e. equal to length / 18.
        let points = size / 18;
        let remaining = size % 18;
        if remaining != 0 {
            return Err(FlacError::InvalidSeekTableSize);
        }

        let mut seek_points = Vec::with_capacity(points);
        for _ in 0..points {
            let sample_number = AsyncReadExt::read_u64(&mut reader).await?;
            let stream_offset = AsyncReadExt::read_u64(&mut reader).await?;
            let frame_samples = AsyncReadExt::read_u16(&mut reader).await?;
            seek_points.push(SeekPoint {
                sample_number,
                stream_offset,
                frame_samples,
            });
        }

        Ok(BlockSeekTable { seek_points })
    }
}

impl Encode for BlockSeekTable {
    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        for point in self.seek_points.iter() {
            writer.write_u64::<BigEndian>(point.sample_number)?;
            writer.write_u64::<BigEndian>(point.stream_offset)?;
            writer.write_u16::<BigEndian>(point.frame_samples)?;
        }
        Ok(())
    }
}

impl fmt::Debug for BlockSeekTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut prefix = "".to_owned();
        if let Some(width) = f.width() {
            prefix = " ".repeat(width);
        }
        writeln!(
            f,
            "{prefix}seek points: {}",
            self.seek_points.len(),
            prefix = prefix
        )?;
        for (i, p) in self.seek_points.iter().enumerate() {
            if p.is_placeholder() {
                writeln!(f, "{prefix}point {}: PLACEHOLDER", i, prefix = prefix)?;
            } else {
                writeln!(
                    f,
                    "{prefix}point {}: sample_number={}, stream_offset={}, frame_samples={}",
                    i,
                    p.sample_number,
                    p.stream_offset,
                    p.frame_samples,
                    prefix = prefix
                )?;
            }
        }
        Ok(())
    }
}
