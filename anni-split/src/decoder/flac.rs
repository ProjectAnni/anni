use crate::decoder::command::{CommandDecoder, INPUT_FILE_PLACEHOLDER};
use crate::decoder::Decoder;
use crate::error::SplitError;
use std::io::Read;
use std::path::Path;

pub struct FlacDecoder;

impl Decoder for FlacDecoder {
    type Output = impl Read;

    fn decode(self, input: impl Read + Send + 'static) -> Result<Self::Output, SplitError> {
        CommandDecoder::new("flac", ["-c", "-d", "-"]).decode(input)
    }

    fn decode_file<P>(self, input: P) -> Result<Self::Output, SplitError>
    where
        P: AsRef<Path>,
    {
        CommandDecoder::new("flac", ["-c", "-d", INPUT_FILE_PLACEHOLDER]).decode_file(input)
    }
}
