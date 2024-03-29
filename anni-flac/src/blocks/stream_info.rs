use crate::prelude::*;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::fmt;
use std::io::{Read, Write};

/// Notes:
/// FLAC specifies a minimum block size of 16 and a maximum block size of 65535,
/// meaning the bit patterns corresponding to the numbers 0-15 in the minimum blocksize and maximum blocksize fields are invalid.
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

#[cfg(feature = "async")]
#[async_trait::async_trait]
impl AsyncDecode for BlockStreamInfo {
    async fn from_async_reader<R>(reader: &mut R) -> Result<Self>
    where
        R: AsyncRead + Unpin + Send,
    {
        use crate::utils::*;

        let min_block_size = reader.read_u16().await?;
        let max_block_size = reader.read_u16().await?;
        let min_frame_size = read_u24_async(reader).await?;
        let max_frame_size = read_u24_async(reader).await?;

        let mut sample_region = [0u8; 8];
        reader.read_exact(&mut sample_region).await?;
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
        reader.read_exact(&mut md5_signature).await?;

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

impl Encode for BlockStreamInfo {
    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_u16::<BigEndian>(self.min_block_size)?;
        writer.write_u16::<BigEndian>(self.max_block_size)?;
        writer.write_u24::<BigEndian>(self.min_frame_size)?;
        writer.write_u24::<BigEndian>(self.max_frame_size)?;

        // 16/20 bits
        writer.write_u16::<BigEndian>((self.sample_rate >> 4) as u16)?;
        let channels = self.channels - 1;
        let bps = self.bits_per_sample - 1;
        writer.write_u48::<BigEndian>(
            // 4 bits of sample rate + 3 bits of channel num + 1 bit bps
            ((((((self.sample_rate & 0b1111) as u8) << 4) + ((channels & 0b111) << 1) + (bps >> 4))
                as u64)
                << 40)
                + (((bps & 0b1111) as u64) << 36)
                + self.total_samples,
        )?;
        writer.write_all(&self.md5_signature)?;
        Ok(())
    }
}

impl fmt::Debug for BlockStreamInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut prefix = "".to_owned();
        if let Some(width) = f.width() {
            prefix = " ".repeat(width);
        }
        writeln!(
            f,
            "{prefix}minimum blocksize: {} samples",
            self.min_block_size,
            prefix = prefix
        )?;
        writeln!(
            f,
            "{prefix}maximum blocksize: {} samples",
            self.max_block_size,
            prefix = prefix
        )?;
        writeln!(
            f,
            "{prefix}minimum framesize: {} bytes",
            self.min_frame_size,
            prefix = prefix
        )?;
        writeln!(
            f,
            "{prefix}maximum framesize: {} bytes",
            self.max_frame_size,
            prefix = prefix
        )?;
        writeln!(
            f,
            "{prefix}sample_rate: {} Hz",
            self.sample_rate,
            prefix = prefix
        )?;
        writeln!(f, "{prefix}channels: {}", self.channels, prefix = prefix)?;
        writeln!(
            f,
            "{prefix}bits-per-sample: {}",
            self.bits_per_sample,
            prefix = prefix
        )?;
        writeln!(
            f,
            "{prefix}total samples: {}",
            self.total_samples,
            prefix = prefix
        )?;
        writeln!(
            f,
            "{prefix}MD5 signature: {}",
            hex::encode(self.md5_signature),
            prefix = prefix
        )?;
        Ok(())
    }
}
