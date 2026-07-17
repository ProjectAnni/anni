use std::{fmt, str::FromStr};

use thiserror::Error;

/// A content digest used to bind manifests, plans, and verification receipts.
///
/// The workflow only needs a fixed-width identity here. Which hashing
/// algorithm produced the bytes belongs to the protocol envelope and can
/// evolve without weakening equality checks inside the state machine.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Digest([u8; 32]);

impl Digest {
    pub const LENGTH: usize = 32;

    pub const fn new(bytes: [u8; Self::LENGTH]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; Self::LENGTH] {
        &self.0
    }
}

impl From<[u8; Digest::LENGTH]> for Digest {
    fn from(value: [u8; Digest::LENGTH]) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Digest({self})")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ParseDigestError {
    #[error("digest must contain exactly 64 hexadecimal characters, got {actual} bytes")]
    InvalidLength { actual: usize },
    #[error("digest contains a non-hexadecimal byte {byte:#04x} at offset {index}")]
    InvalidByte { index: usize, byte: u8 },
}

impl FromStr for Digest {
    type Err = ParseDigestError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let encoded = value.as_bytes();
        if encoded.len() != Self::LENGTH * 2 {
            return Err(ParseDigestError::InvalidLength {
                actual: encoded.len(),
            });
        }

        let mut decoded = [0; Self::LENGTH];
        for (index, pair) in encoded.chunks_exact(2).enumerate() {
            let high = decode_nibble(pair[0], index * 2)?;
            let low = decode_nibble(pair[1], index * 2 + 1)?;
            decoded[index] = (high << 4) | low;
        }
        Ok(Self::new(decoded))
    }
}

fn decode_nibble(byte: u8, index: usize) -> Result<u8, ParseDigestError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(ParseDigestError::InvalidByte { index, byte }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_hex_round_trip_is_strict_and_case_insensitive() {
        let digest = Digest::new([0xab; Digest::LENGTH]);
        let encoded = digest.to_string();

        assert_eq!(encoded.parse(), Ok(digest));
        assert_eq!(encoded.to_uppercase().parse(), Ok(digest));
        assert_eq!(
            "ab".parse::<Digest>(),
            Err(ParseDigestError::InvalidLength { actual: 2 })
        );

        let invalid = format!("{}z", &encoded[..encoded.len() - 1]);
        assert_eq!(
            invalid.parse::<Digest>(),
            Err(ParseDigestError::InvalidByte {
                index: 63,
                byte: b'z',
            })
        );
    }
}
