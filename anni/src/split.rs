use std::io::{Write, Read};
use anni_common::Decode;
use anni_utils::decode;
use anni_utils::decode::{u32_be, u32_le, u16_le, DecodeError};
use std::fs::File;

#[derive(Debug)]
struct WaveHeader {
    audio_format: u16,
    channels: u16,
    sample_rate: u32,
    byte_rate: u32,
    block_align: u16,
    bit_per_sample: u16,
}

impl Decode for WaveHeader {
    type Err = DecodeError;

    fn from_reader<R: Read>(reader: &mut R) -> Result<Self, Self::Err> {
        // RIFF chunk
        decode::token(reader, b"RIFF")?;
        let _chunk_size = u32_le(reader)?;
        decode::token(reader, b"WAVE")?;

        // fmt sub-chunk
        decode::token(reader, b"fmt ")?;
        let _fmt_size = u32_le(reader)?;
        let audio_format = u16_le(reader)?;
        let channels = u16_le(reader)?;
        let sample_rate = u32_le(reader)?;
        let byte_rate = u32_le(reader)?;
        let block_align = u16_le(reader)?;
        let bit_per_sample = u16_le(reader)?;

        // data sub-chunk
        decode::token(reader, b"data")?;
        Ok(WaveHeader {
            audio_format,
            channels,
            sample_rate,
            byte_rate,
            block_align,
            bit_per_sample,
        })
    }
}

fn mmssff(bps: usize, m: usize, s: usize, f: usize) -> usize {
    m * bps * 60 + s * bps + f * bps / 75
}

#[test]
fn test_file() {
    let mut file = File::open("/home/yesterday17/公共/[161109][KICM-1726] Starry Wish/水瀬いのり - Starry Wish.wav").unwrap();
    let header = WaveHeader::from_reader(&mut file).unwrap();
    // let start = mmssff(spec.bits_per_sample, 4, 7, 48) * spec.channels;
    // let end = mmssff(spec.bits_per_sample, 8, 17, 49) * spec.channels;
}