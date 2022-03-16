use std::borrow::Cow;
use crate::providers;
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

/// Backend is a common trait for anni backends.
/// It provides functions to update albums, and read from an initialized backend.
#[async_trait]
pub trait AnniProvider {
    /// Get album information provided by backend.
    async fn albums(&self) -> Result<HashSet<Cow<str>>, ProviderError>;

    /// Returns a reader implements AsyncRead for content reading
    async fn get_audio(&self, album_id: &str, disc_id: u8, track_id: u8, range: Option<String>) -> Result<AudioResourceReader, ProviderError>;

    /// Returns a cover of corresponding album
    async fn get_cover(&self, album_id: &str, disc_id: Option<u8>) -> Result<ResourceReader, ProviderError>;

    /// Reloads the backend for new albums
    async fn reload(&mut self) -> Result<(), ProviderError>;
}

pub enum AnniBackend {
    File(providers::FileBackend),
    Drive(providers::DriveBackend),
    Cache(crate::cache::Cache),
}

impl AnniBackend {
    pub fn into_box(self) -> Box<dyn AnniProvider + Send + Sync> {
        match self {
            AnniBackend::File(b) => Box::new(b),
            AnniBackend::Drive(b) => Box::new(b),
            AnniBackend::Cache(b) => Box::new(b),
        }
    }

    pub fn as_backend(&self) -> &dyn AnniProvider {
        match self {
            AnniBackend::File(b) => b,
            AnniBackend::Drive(b) => b,
            AnniBackend::Cache(b) => b,
        }
    }

    pub fn as_backend_mut(&mut self) -> &mut dyn AnniProvider {
        match self {
            AnniBackend::File(b) => b,
            AnniBackend::Drive(b) => b,
            AnniBackend::Cache(b) => b,
        }
    }

    pub async fn contains_album(&self, album_id: &str) -> bool {
        self.as_backend().albums().await.unwrap_or(HashSet::new()).contains(album_id)
    }
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
