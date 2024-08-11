use std::fmt::Display;

use reqwest::blocking::{Client, Response};

use anni_common::models::RawTrackIdentifier;

#[derive(Debug, Copy, Clone)]
pub enum AudioQuality {
    Low,
    Medium,
    High,
    Lossless,
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
        format!(
            "{}/{}?auth={}&quality={}&opus={}",
            self.url, track, self.auth, quality, opus
        )
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
            .get(self.format_url(track, quality, opus))
            // .header("Authorization", &self.auth)
            .send()
    }
}
