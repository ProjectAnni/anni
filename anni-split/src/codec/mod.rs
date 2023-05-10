pub mod command;
pub mod wav;

/// [Decoder] trait to decode from specified format to WAVE.
pub trait Decoder: Sized {
    type Output: std::io::Read + Send;

    fn decode(self) -> Result<Self::Output, crate::SplitError>;
}

/// [Encoder] trait to encoder from WAVE to specified format.
pub trait Encoder: Sized {
    fn encode(self, input: impl std::io::Read) -> Result<(), crate::SplitError>;
}

// Command En/Decoders
use crate::codec::command::FILE_PLACEHOLDER;
use crate::{command_decoder, command_encoder};

command_decoder!(FlacCommandDecoder, "flac", ["-c", "-d", FILE_PLACEHOLDER]);
command_encoder!(
    FlacCommandEncoder,
    "flac",
    ["--totally-silent", "-", "-o", FILE_PLACEHOLDER]
);
command_decoder!(ApeCommandDecoder, "mac", [FILE_PLACEHOLDER, "-", "-d"]);
command_decoder!(TakCommandDecoder, "takc", ["-d", FILE_PLACEHOLDER, "-"]);
command_decoder!(
    TtaCommandDecoder,
    "ttaenc",
    ["-d", "-o", "-", FILE_PLACEHOLDER]
);

#[cfg(test)]
mod tests {
    use crate::codec::wav::WavEncoder;
    use crate::codec::{Decoder, FlacCommandDecoder};
    use crate::{Encoder, SplitError};

    #[test]
    fn test_decode_flac() -> Result<(), SplitError> {
        let decoded = FlacCommandDecoder("/tmp/test.flac").decode()?;
        WavEncoder("/tmp/result.wav").encode(decoded)?;
        Ok(())
    }
}
