use std::io::Read;
use byteorder::{ReadBytesExt, BigEndian};
use crate::error::FlacError;
use crate::prelude::{Decode, Result};
use crate::utils::take_to_end;

#[derive(Debug)]
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
    pub fn is_placehoder(&self) -> bool {
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
            let sample_number = reader.read_u64::<BigEndian>()?;
            let stream_offset = reader.read_u64::<BigEndian>()?;
            let frame_samples = reader.read_u16::<BigEndian>()?;
            seek_points.push(SeekPoint {
                sample_number,
                stream_offset,
                frame_samples,
            });
        }

        Ok(BlockSeekTable { seek_points })
    }
}