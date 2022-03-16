use std::borrow::Cow;
use async_trait::async_trait;
use std::collections::HashSet;
use std::pin::Pin;
use thiserror::Error;
use tokio::io::AsyncRead;

pub type ResourceReader = Pin<Box<dyn AsyncRead + Send>>;

/// AudioResourceReader abstracts the file result a provider returns with extra information of audio
pub struct AudioResourceReader {
    /// File extension of the file
    pub extension: String,
    /// File size of the file
    pub size: usize,
    /// Audio duration of the file
    pub duration: u64,
    /// Optional file range
    pub range: Option<String>,
    /// Async Reader for the file
    pub reader: ResourceReader,
}

/// AnniProvider is a common trait for anni resource providers.
/// It provides functions to get cover, audio, album list and reload.
#[async_trait]
pub trait AnniProvider {
    /// Get album information provided by provider.
    async fn albums(&self) -> Result<HashSet<Cow<str>>, ProviderError>;

    /// Returns a reader implements AsyncRead for content reading
    async fn get_audio(&self, album_id: &str, disc_id: u8, track_id: u8, range: Option<String>) -> Result<AudioResourceReader, ProviderError>;

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
