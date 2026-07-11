use std::{fmt::Display, str::FromStr};

use reqwest::{
    blocking::{Client, Response},
    Url,
};

use anni_common::models::RawTrackIdentifier;

#[derive(Debug, Copy, Clone, Default, Hash, PartialEq, Eq)]
pub enum AudioQuality {
    Low,
    #[default]
    Medium,
    High,
    Lossless,
}

impl AudioQuality {
    /// The annil transcoder's target bitrate for lossy qualities.
    pub const fn bitrate_kbps(self) -> Option<u16> {
        match self {
            Self::Low => Some(128),
            Self::Medium => Some(192),
            Self::High => Some(256),
            Self::Lossless => None,
        }
    }
}

impl FromStr for AudioQuality {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            "lossless" => Ok(Self::Lossless),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Copy, Clone, Default, Hash, PartialEq, Eq)]
pub enum AudioCodec {
    Original,
    #[default]
    Aac,
    Opus,
}

impl Display for AudioCodec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Original => write!(f, "original"),
            Self::Aac => write!(f, "aac"),
            Self::Opus => write!(f, "opus"),
        }
    }
}

/// The actual representation requested from annil and used as part of the cache key.
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub struct AudioVariant {
    quality: AudioQuality,
    codec: AudioCodec,
}

impl AudioVariant {
    pub fn new(quality: AudioQuality, codec: AudioCodec) -> Self {
        let codec = match (quality, codec) {
            (AudioQuality::Lossless, _) => AudioCodec::Original,
            (_, AudioCodec::Original) => AudioCodec::Aac,
            (_, codec) => codec,
        };
        Self { quality, codec }
    }

    pub const fn quality(self) -> AudioQuality {
        self.quality
    }

    pub const fn codec(self) -> AudioCodec {
        self.codec
    }

    pub fn from_legacy(quality: AudioQuality, opus: bool) -> Self {
        Self::new(
            quality,
            if opus {
                AudioCodec::Opus
            } else {
                AudioCodec::Aac
            },
        )
    }

    pub fn uses_opus(self) -> bool {
        self.codec == AudioCodec::Opus
    }

    pub(crate) fn cache_suffix(self) -> String {
        format!("{}-{}", self.quality, self.codec)
    }
}

impl Default for AudioVariant {
    fn default() -> Self {
        Self::new(AudioQuality::Medium, AudioCodec::Aac)
    }
}

impl Display for AudioQuality {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioQuality::Low => write!(f, "low"),
            AudioQuality::Medium => write!(f, "medium"),
            AudioQuality::High => write!(f, "high"),
            AudioQuality::Lossless => write!(f, "lossless"),
        }
    }
}

pub struct ProviderProxy {
    url: String,
    client: Client,
    auth: String,
}

impl ProviderProxy {
    pub fn new(url: String, auth: String, client: Client) -> Self {
        Self { url, auth, client }
    }

    pub fn format_url(
        &self,
        track: RawTrackIdentifier,
        quality: AudioQuality,
        opus: bool,
    ) -> String {
        self.format_variant_url(track, AudioVariant::from_legacy(quality, opus))
    }

    pub fn format_variant_url(&self, track: RawTrackIdentifier, variant: AudioVariant) -> String {
        let base = format!("{}/{}", self.url.trim_end_matches('/'), track);
        let Ok(mut url) = Url::parse(&base) else {
            return format!(
                "{base}?auth={}&quality={}&opus={}",
                self.auth,
                variant.quality,
                variant.uses_opus()
            );
        };
        url.query_pairs_mut()
            .append_pair("auth", &self.auth)
            .append_pair("quality", &variant.quality.to_string())
            .append_pair("opus", &variant.uses_opus().to_string());
        url.into()
    }

    pub fn get(
        &self,
        track: RawTrackIdentifier,
        quality: AudioQuality,
        opus: bool,
    ) -> reqwest::Result<Response> {
        self.client
            .get(self.format_url(track, quality, opus))
            .send()
    }

    pub fn head(
        &self,
        track: RawTrackIdentifier,
        quality: AudioQuality,
        opus: bool,
    ) -> reqwest::Result<Response> {
        self.client
            .head(self.format_url(track, quality, opus))
            .send()
    }

    pub fn head_with_client(
        &self,
        client: &Client,
        track: RawTrackIdentifier,
        quality: AudioQuality,
        opus: bool,
    ) -> reqwest::Result<Response> {
        client.head(self.format_url(track, quality, opus)).send()
    }
}

#[cfg(test)]
mod tests {
    use super::{AudioCodec, AudioQuality, AudioVariant};

    #[test]
    fn variants_canonicalize_codec_and_expose_annil_bitrate() {
        let lossy = AudioVariant::new(AudioQuality::Low, AudioCodec::Original);
        assert_eq!(lossy.codec(), AudioCodec::Aac);
        assert_eq!(lossy.quality().bitrate_kbps(), Some(128));

        let lossless = AudioVariant::new(AudioQuality::Lossless, AudioCodec::Opus);
        assert_eq!(lossless.codec(), AudioCodec::Original);
        assert_eq!(lossless.quality().bitrate_kbps(), None);
    }
}
