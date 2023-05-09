pub mod ape;
pub mod command;
pub mod flac;
pub mod tak;
pub mod tta;

use crate::error::SplitError;
use std::io::Read;
use std::path::Path;

/// [Decoder] trait to decode from specified format to WAVE.
pub trait Decoder: Sized {
    type Output: Read;

    fn decode(self, input: impl Read + Send + 'static) -> Result<Self::Output, SplitError>;

    fn decode_file<P>(self, input: P) -> Result<Self::Output, SplitError>
    where
        P: AsRef<Path>,
    {
        let input = std::fs::File::open(input)?;
        self.decode(input)
    }
}

pub trait Encoder: Sized {
    fn encode<P>(self, input: impl Read + Send + 'static, output: P) -> Result<(), SplitError>
    where
        P: AsRef<Path>;
}

#[cfg(test)]
mod tests {
    use crate::codec::flac::FlacDecoder;
    use crate::codec::Decoder;
    use crate::error::SplitError;
    use std::fs::File;

    #[test]
    fn test_decode_flac() -> Result<(), SplitError> {
        let mut output = FlacDecoder.decode_file("/tmp/test.flac")?;
        let mut out_file = File::create("/tmp/result.wav")?;
        std::io::copy(&mut output, &mut out_file)?;
        Ok(())
    }
}
