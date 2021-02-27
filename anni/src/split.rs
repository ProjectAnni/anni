use std::io::{Write, Read, Seek, SeekFrom};
use anni_common::{Decode, Encode};
use anni_utils::decode;
use anni_utils::decode::{u32_le, u16_le, DecodeError};
use std::fs::File;
use anni_utils::encode::{btoken_w, u32_le_w, u16_le_w};

#[derive(Debug)]
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

    fn from_reader<R: Read>(reader: &mut R) -> Result<Self, Self::Err> {
        // RIFF chunk
        decode::token(reader, b"RIFF")?;
        let _chunk_size = u32_le(reader)?;
        debug!(target: "wav", "RIFF chunk detected, size = {size}", size = _chunk_size);
        decode::token(reader, b"WAVE")?;

        // fmt sub-chunk
        decode::token(reader, b"fmt ")?;
        let _fmt_size = u32_le(reader)?;
        debug!(target: "wav", "Chunk [fmt ] found, size = {size}", size = _fmt_size);

        let audio_format = u16_le(reader)?;
        if audio_format != 1 {
            error!(target: "wav", "Only PCM format(1) is supported for now, got {}", audio_format);
            return Err(DecodeError::InvalidTokenError { expected: b"1".to_vec(), got: vec![(audio_format >> 8) as u8, (audio_format & 0xff) as u8] });
        }

        let channels = u16_le(reader)?;
        let sample_rate = u32_le(reader)?;
        let byte_rate = u32_le(reader)?;
        let block_align = u16_le(reader)?;
        let bit_per_sample = u16_le(reader)?;
        debug!(target: "wav", "  channles = {}", channels);
        debug!(target: "wav", "  sample_rate = {}", sample_rate);
        debug!(target: "wav", "  byte_rate = {}", byte_rate);
        debug!(target: "wav", "  block_alibn = {}", block_align);
        debug!(target: "wav", "  bit_per_sample = {}", bit_per_sample);

        // data sub-chunk
        decode::token(reader, b"data")?;
        let data_size = u32_le(reader)?;
        debug!(target: "wav", "Chunk [data] found, size = {size}", size = data_size);
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
    pub fn is_cd(&self) -> bool {
        self.sample_rate == 44100
    }

    pub fn mmssff(&self, m: usize, s: usize, f: usize) -> usize {
        let br = self.byte_rate as usize;
        br * 60 * m + br * s + br * f / 75
    }

    pub fn mmssnnn(&self, m: usize, s: usize, n: usize) -> usize {
        unimplemented!()
    }
}

pub fn split_wav(header: &mut WaveHeader, input: &mut File, mut output: File, start: usize, end: usize) {
    input.seek(SeekFrom::Start(44 + start as u64));
    let size = end - start;
    header.data_size = size as u32;
    header.write_to(&mut output).unwrap();
    std::io::copy(&mut input.take(size as u64), &mut output).unwrap();
}

#[test]
fn test_split() {
    let mut file = File::open("/home/yesterday17/公共/[161109][KICM-1726] Starry Wish/水瀬いのり - Starry Wish.wav").unwrap();
    let mut header = WaveHeader::from_reader(&mut file).unwrap();

    let one = header.mmssff(4, 7, 48);
    let two = header.mmssff(8, 17, 49);
    let end = header.data_size as usize;

    let mut output = File::create("/home/yesterday17/公共/[161109][KICM-1726] Starry Wish/s1.wav").unwrap();
    split_wav(&mut header, &mut file, output, 0, one);
    let mut output = File::create("/home/yesterday17/公共/[161109][KICM-1726] Starry Wish/s2.wav").unwrap();
    split_wav(&mut header, &mut file, output, one, two);
    let mut output = File::create("/home/yesterday17/公共/[161109][KICM-1726] Starry Wish/s3.wav").unwrap();
    split_wav(&mut header, &mut file, output, two, end);
}