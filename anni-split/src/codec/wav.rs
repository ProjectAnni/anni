use anni_common::decode::{token, u16_le, u32_le, DecodeError};
use anni_common::encode::{btoken_w, u16_le_w, u32_le_w};
use anni_common::traits::{Decode, Encode};
use log::{debug, error};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use super::{Decoder, Encoder};

#[derive(Debug, Clone)]
pub struct WaveHeader {
    pub channels: u16,
    pub sample_rate: u32,
    pub byte_rate: u32,
    pub block_align: u16,
    pub bit_per_sample: u16,
    pub data_size: u32,
}

impl Decode for WaveHeader {
    type Err = DecodeError;

    fn from_reader<R>(reader: &mut R) -> Result<Self, Self::Err>
    where
        R: Read,
    {
        // RIFF chunk
        token(reader, b"RIFF")?;
        let _chunk_size = u32_le(reader)?;
        debug!("RIFF chunk detected, size = {size}", size = _chunk_size);
        token(reader, b"WAVE")?;

        // fmt sub-chunk
        token(reader, b"fmt ")?;
        let _fmt_size = u32_le(reader)?;
        debug!("Chunk [fmt ] found, size = {size}", size = _fmt_size);

        let audio_format = u16_le(reader)?;
        if audio_format != 1 {
            error!(
                "Only PCM format(1) is supported for now, got {}",
                audio_format
            );
            return Err(DecodeError::InvalidTokenError {
                expected: b"1".to_vec(),
                got: vec![(audio_format >> 8) as u8, (audio_format & 0xff) as u8],
            });
        }

        let channels = u16_le(reader)?;
        let sample_rate = u32_le(reader)?;
        let byte_rate = u32_le(reader)?;
        let block_align = u16_le(reader)?;
        let bit_per_sample = u16_le(reader)?;
        debug!("  channels = {}", channels);
        debug!("  sample_rate = {}", sample_rate);
        debug!("  byte_rate = {}", byte_rate);
        debug!("  block_align = {}", block_align);
        debug!("  bit_per_sample = {}", bit_per_sample);

        // data sub-chunk
        token(reader, b"data")?;
        let data_size = u32_le(reader)?;
        debug!("Chunk [data] found, size = {size}", size = data_size);
        Ok(WaveHeader {
            channels,
            sample_rate,
            byte_rate,
            block_align,
            bit_per_sample,
            data_size,
        })
    }
}

impl Encode for WaveHeader {
    type Err = std::io::Error;

    fn write_to<W: Write>(&self, writer: &mut W) -> Result<(), Self::Err> {
        btoken_w(writer, b"RIFF")?;
        u32_le_w(writer, self.data_size + 16)?; // chunk size
        btoken_w(writer, b"WAVE")?;
        btoken_w(writer, b"fmt ")?;
        u32_le_w(writer, 16)?; // PCM chunk size
        u16_le_w(writer, 1)?; // audio format = 1, PCM
        u16_le_w(writer, self.channels)?;
        u32_le_w(writer, self.sample_rate)?;
        u32_le_w(writer, self.byte_rate)?;
        u16_le_w(writer, self.block_align)?;
        u16_le_w(writer, self.bit_per_sample)?;
        btoken_w(writer, b"data")?;
        u32_le_w(writer, self.data_size)?;
        Ok(())
    }
}

impl WaveHeader {
    pub fn offset_from_second_frames(&self, s: u32, f: u32) -> u32 {
        let br = self.byte_rate;
        br * s + br * f / 75
    }
}

pub struct WavDecoder<P: AsRef<Path>>(pub P);

impl<P: AsRef<Path>> Decoder for WavDecoder<P> {
    type Output = impl Read;

    fn decode(self) -> Result<Self::Output, crate::error::SplitError> {
        Ok(File::open(self.0)?)
    }
}

pub struct WavEncoder<P: AsRef<Path>>(pub P);

impl<P: AsRef<Path>> Encoder for WavEncoder<P> {
    fn encode(self, mut input: impl Read) -> Result<(), crate::error::SplitError> {
        let mut output = File::open(self.0)?;
        std::io::copy(&mut input, &mut output)?;
        Ok(())
    }
}
