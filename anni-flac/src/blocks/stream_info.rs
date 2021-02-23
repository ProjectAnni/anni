use crate::{Decode, Result};
use std::io::Read;
use byteorder::{ReadBytesExt, BigEndian};

/// Notes:
/// FLAC specifies a minimum block size of 16 and a maximum block size of 65535,
/// meaning the bit patterns corresponding to the numbers 0-15 in the minimum blocksize and maximum blocksize fields are invalid.
#[derive(Debug)]
pub struct BlockStreamInfo {
    /// <16> The minimum block size (in samples) used in the stream.
    pub min_block_size: u16,
    /// <16> The maximum block size (in samples) used in the stream.
    pub max_block_size: u16,
    /// <24> The minimum frame size (in bytes) used in the stream. May be 0 to imply the value is not known.
    pub min_frame_size: u32,
    /// <24> The maximum frame size (in bytes) used in the stream. May be 0 to imply the value is not known.
    pub max_frame_size: u32,
    /// <20> Sample rate in Hz.
    /// Though 20 bits are available, the maximum sample rate is limited by the structure of frame headers to 655350Hz.
    /// Also, a value of 0 is invalid.
    pub sample_rate: u32,
    /// <3> (number of channels)-1.
    /// FLAC supports from 1 to 8 channels
    pub channels: u8,
    /// <5> (bits per sample)-1.
    /// FLAC supports from 4 to 32 bits per sample.
    /// Currently the reference encoder and decoders only support up to 24 bits per sample.
    pub bits_per_sample: u8,
    /// <36> Total samples in stream.
    /// 'Samples' means inter-channel sample, i.e. one second of 44.1Khz audio will have 44100 samples regardless of the number of channels.
    /// A value of zero here means the number of total samples is unknown.
    pub total_samples: u64,
    /// <128> MD5 signature of the unencoded audio data.
    /// This allows the decoder to determine if an error exists in the audio data even when the error does not result in an invalid bitstream.
    pub md5_signature: [u8; 16],
}

impl BlockStreamInfo {
    /// (Minimum blocksize == maximum blocksize) implies a fixed-blocksize stream.
    pub fn is_fixed_blocksize_stream(&self) -> bool {
        self.min_block_size == self.max_block_size
    }
}

impl Decode for BlockStreamInfo {
    fn from_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let min_block_size = reader.read_u16::<BigEndian>()?;
        let max_block_size = reader.read_u16::<BigEndian>()?;
        let min_frame_size = reader.read_u24::<BigEndian>()?;
        let max_frame_size = reader.read_u24::<BigEndian>()?;

        let mut sample_region = [0u8; 8];
        reader.read_exact(&mut sample_region)?;
        // 20 bits
        let sample_rate = ((sample_region[0] as u32) << 12)
            + ((sample_region[1] as u32) << 4)
            + ((sample_region[2] as u32) >> 4);
        // 3 bits
        let channels = ((sample_region[2] >> 1) & 0b00000111) + 1;
        // 5 bits
        let bits_per_sample = ((sample_region[2] & 0b00000001) << 4) + (sample_region[3] >> 4) + 1;
        // 36 bits
        let total_samples = ((sample_region[3] as u64 & 0b00001111) << 32)
            + ((sample_region[4] as u64) << 24)
            + ((sample_region[5] as u64) << 16)
            + ((sample_region[6] as u64) << 8)
            + (sample_region[7] as u64);
        let mut md5_signature = [0u8; 16];
        reader.read_exact(&mut md5_signature)?;

        Ok(BlockStreamInfo {
            min_block_size,
            max_block_size,
            min_frame_size,
            max_frame_size,
            sample_rate,
            channels,
            bits_per_sample,
            total_samples,
            md5_signature,
        })
    }
}
