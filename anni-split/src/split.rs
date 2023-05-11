use anni_common::traits::{Decode, Encode};
use std::io::{Cursor, Read};

use crate::{
    codec::{wav::WaveHeader, Decoder, Encoder},
    error::SplitError,
};

pub fn split<F, E, I, B>(input: impl Decoder, output: F, breakpoints: I) -> Result<(), SplitError>
where
    F: Fn(usize) -> Result<E, SplitError>,
    E: Encoder + 'static,
    I: IntoIterator<Item = B>,
    B: Breakpoint,
{
    let mut reader = &mut input.decode()?;
    let header = WaveHeader::from_reader(&mut reader).unwrap();

    let mut start = 0u32;

    for (index, end) in breakpoints
        .into_iter()
        .map(|b| b.position(&header))
        .chain([header.data_size])
        .enumerate()
    {
        let encoder = output(index)?;
        let size = end - start;

        let mut header_buf = Cursor::new([0; 44]);
        let mut header = header.clone();
        header.data_size = size;
        header.write_to(&mut header_buf)?;
        header_buf.set_position(0);

        let body = &mut reader.take(size as u64);
        encoder.encode(header_buf.chain(body))?;

        start = end;
    }

    Ok(())
}

pub trait Breakpoint {
    fn position(&self, header: &WaveHeader) -> u32;
}

pub struct RawBreakpoint(u32);

impl Breakpoint for RawBreakpoint {
    fn position(&self, _: &WaveHeader) -> u32 {
        self.0
    }
}
