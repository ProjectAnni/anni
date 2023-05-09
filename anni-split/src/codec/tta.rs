use crate::codec::command::{CommandCodec, FILE_PLACEHOLDER};
use crate::codec::Decoder;
use crate::error::SplitError;
use std::io::Read;
use std::path::Path;

pub struct TakDecoder;

impl Decoder for TakDecoder {
    type Output = impl Read;

    fn decode(self, input: impl Read + Send + 'static) -> Result<Self::Output, SplitError> {
        CommandCodec::new("ttaenc", ["-d", "-o", "-", "-"])?.decode(input)
    }

    fn decode_file<P>(self, input: P) -> Result<Self::Output, SplitError>
    where
        P: AsRef<Path>,
    {
        CommandCodec::new("ttaenc", ["-d", "-o", "-", FILE_PLACEHOLDER])?.decode_file(input)
    }
}
