use std::borrow::Cow;
use async_trait::async_trait;
use std::collections::HashSet;
use std::pin::Pin;
use thiserror::Error;
use tokio::io::AsyncRead;

pub type ResourceReader = Pin<Box<dyn AsyncRead + Send>>;

pub struct AudioInfo {
    /// File extension of the file
    pub extension: String,
    /// File size of the file
    pub size: usize,
    /// Audio duration of the file
    pub duration: u64,
}

/// AudioResourceReader abstracts the file result a provider returns with extra information of audio
pub struct AudioResourceReader {
    /// Audio info
    pub info: AudioInfo,
    /// File range
    pub range: Range,
    /// Async Reader for the file
    pub reader: ResourceReader,
}

#[derive(Clone)]
pub struct Range {
    pub start: u64,
    pub end: Option<u64>,
}

impl Range {
    pub fn new(start: u64, end: Option<u64>) -> Self {
        Self { start, end }
    }

    pub fn full() -> Self {
        Self { start: 0, end: None }
    }

    pub fn start(&self) -> u64 {
        self.start
    }

    pub fn length(&self) -> u64 {
        match self.end {
            Some(end) => end - self.start + 1,
            None => 0,
        }
    }
}

/// AnniProvider is a common trait for anni resource providers.
/// It provides functions to get cover, audio, album list and reload.
#[async_trait]
pub trait AnniProvider {
    /// Get album information provided by provider.
    async fn albums(&self) -> Result<HashSet<Cow<str>>, ProviderError>;

    /// Get audio info describing basic information of the audio file.
    async fn get_audio_info(&self, album_id: &str, disc_id: u8, track_id: u8) -> Result<AudioInfo, ProviderError>;

    /// Returns a reader implements AsyncRead for content reading
    async fn get_audio(&self, album_id: &str, disc_id: u8, track_id: u8, range: Range) -> Result<AudioResourceReader, ProviderError>;

    /// Returns a cover of corresponding album
    async fn get_cover(&self, album_id: &str, disc_id: Option<u8>) -> Result<ResourceReader, ProviderError>;

    /// Reloads the provider for new albums
    async fn reload(&mut self) -> Result<(), ProviderError>;
}

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("invalid path")]
    InvalidPath,

    #[error("file not found")]
    FileNotFound,

    #[error(transparent)]
    IOError(#[from] std::io::Error),

    #[error(transparent)]
    RepoError(#[from] anni_repo::error::Error),

    #[error(transparent)]
    OAuthError(#[from] yup_oauth2::Error),

    #[error(transparent)]
    DriveError(#[from] google_drive3::Error),

    #[error(transparent)]
    RequestError(#[from] reqwest::Error),

    #[error(transparent)]
    FlacError(#[from] anni_flac::error::FlacError),
}
