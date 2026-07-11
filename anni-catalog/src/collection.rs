use std::{fmt, num::NonZeroU32, str::FromStr};

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CollectionState {
    Missing,
    Wanted,
    Acquired,
    Ingesting,
    Published,
    Unavailable,
}

impl CollectionState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Wanted => "wanted",
            Self::Acquired => "acquired",
            Self::Ingesting => "ingesting",
            Self::Published => "published",
            Self::Unavailable => "unavailable",
        }
    }
}

impl fmt::Display for CollectionState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for CollectionState {
    type Err = UnknownCollectionState;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "missing" => Ok(Self::Missing),
            "wanted" => Ok(Self::Wanted),
            "acquired" => Ok(Self::Acquired),
            "ingesting" => Ok(Self::Ingesting),
            "published" => Ok(Self::Published),
            "unavailable" => Ok(Self::Unavailable),
            _ => Err(UnknownCollectionState(value.to_owned())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("unknown collection state: {0}")]
pub struct UnknownCollectionState(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioCodec {
    Flac,
    Wav,
    Alac,
    Aac,
    Mp3,
    Opus,
    Other,
}

impl AudioCodec {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Flac => "flac",
            Self::Wav => "wav",
            Self::Alac => "alac",
            Self::Aac => "aac",
            Self::Mp3 => "mp3",
            Self::Opus => "opus",
            Self::Other => "other",
        }
    }

    pub const fn is_lossless(self) -> Option<bool> {
        match self {
            Self::Flac | Self::Wav | Self::Alac => Some(true),
            Self::Aac | Self::Mp3 | Self::Opus => Some(false),
            Self::Other => None,
        }
    }
}

impl fmt::Display for AudioCodec {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for AudioCodec {
    type Err = UnknownAudioCodec;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "flac" => Ok(Self::Flac),
            "wav" => Ok(Self::Wav),
            "alac" => Ok(Self::Alac),
            "aac" => Ok(Self::Aac),
            "mp3" => Ok(Self::Mp3),
            "opus" => Ok(Self::Opus),
            "other" => Ok(Self::Other),
            _ => Err(UnknownAudioCodec(value.to_owned())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("unknown audio codec: {0}")]
pub struct UnknownAudioCodec(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QualityTier {
    Unknown,
    Lossy,
    Lossless,
    HiResLossless,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AudioProperties {
    codec: AudioCodec,
    sample_rate_hz: Option<NonZeroU32>,
    bit_depth: Option<u8>,
    channels: Option<u8>,
}

impl AudioProperties {
    pub fn new(
        codec: AudioCodec,
        sample_rate_hz: Option<NonZeroU32>,
        bit_depth: Option<u8>,
        channels: Option<u8>,
    ) -> Result<Self, AudioPropertiesError> {
        if bit_depth == Some(0) {
            return Err(AudioPropertiesError::ZeroBitDepth);
        }
        if channels == Some(0) {
            return Err(AudioPropertiesError::ZeroChannels);
        }
        Ok(Self {
            codec,
            sample_rate_hz,
            bit_depth,
            channels,
        })
    }

    pub const fn codec(self) -> AudioCodec {
        self.codec
    }

    pub const fn sample_rate_hz(self) -> Option<NonZeroU32> {
        self.sample_rate_hz
    }

    pub const fn bit_depth(self) -> Option<u8> {
        self.bit_depth
    }

    pub const fn channels(self) -> Option<u8> {
        self.channels
    }

    pub const fn quality_tier(self) -> QualityTier {
        match self.codec.is_lossless() {
            Some(false) => QualityTier::Lossy,
            None => QualityTier::Unknown,
            Some(true)
                if matches!(self.bit_depth, Some(depth) if depth > 16)
                    || matches!(self.sample_rate_hz, Some(rate) if rate.get() > 48_000) =>
            {
                QualityTier::HiResLossless
            }
            Some(true) => QualityTier::Lossless,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum AudioPropertiesError {
    #[error("bit depth must be greater than zero when known")]
    ZeroBitDepth,
    #[error("channel count must be greater than zero when known")]
    ZeroChannels,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quality_tier_uses_codec_and_measured_properties() {
        let cd = AudioProperties::new(AudioCodec::Flac, NonZeroU32::new(44_100), Some(16), Some(2))
            .unwrap();
        let hi_res =
            AudioProperties::new(AudioCodec::Flac, NonZeroU32::new(96_000), Some(24), Some(2))
                .unwrap();
        let lossy =
            AudioProperties::new(AudioCodec::Aac, NonZeroU32::new(48_000), None, Some(2)).unwrap();

        assert_eq!(cd.quality_tier(), QualityTier::Lossless);
        assert_eq!(hi_res.quality_tier(), QualityTier::HiResLossless);
        assert_eq!(lossy.quality_tier(), QualityTier::Lossy);
    }
}
