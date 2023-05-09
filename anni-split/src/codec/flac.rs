use crate::codec::command::{CommandCodec, FILE_PLACEHOLDER};
use crate::codec::{Decoder, Encoder};
use crate::error::SplitError;
use std::io::Read;
use std::path::Path;

pub struct FlacDecoder;

impl Decoder for FlacDecoder {
    type Output = impl Read;

    fn decode(self, input: impl Read + Send + 'static) -> Result<Self::Output, SplitError> {
        CommandCodec::new("flac", ["-c", "-d", "-"])?.decode(input)
    }

    fn decode_file<P>(self, input: P) -> Result<Self::Output, SplitError>
    where
        P: AsRef<Path>,
    {
        CommandCodec::new("flac", ["-c", "-d", FILE_PLACEHOLDER])?.decode_file(input)
    }
}

pub struct FlacEncoder;

impl Encoder for FlacEncoder {
    fn encode<P>(self, input: impl Read + Send + 'static, output: P) -> Result<(), SplitError>
    where
        P: AsRef<Path>,
    {
        CommandCodec::new("flac", ["--totally-silent", "-", "-o", FILE_PLACEHOLDER])?
            .encode(input, output)
    }
}
