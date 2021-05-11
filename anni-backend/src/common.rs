use crate::backends;
use async_trait::async_trait;
use std::collections::HashSet;
use std::pin::Pin;
use thiserror::Error;
use tokio::io::AsyncRead;

/// BackendAudio abstracts the audio result a backend returns when `get_audio` is called
pub struct BackendAudio {
    /// File extension of audio file
    pub extension: String,
    /// File size of audio file
    pub size: u64,
    /// Async Reader for audio file
    pub reader: Pin<Box<dyn AsyncRead>>,
}

/// Backend is a common trait for anni backends.
/// It provides functions to update albums, and read from an initialized backend.
#[async_trait]
pub trait Backend {
    /// Get album information provided by backend.
    async fn albums(&mut self) -> Result<HashSet<String>, BackendError>;

    /// Returns a reader implements AsyncRead for content reading
    async fn get_audio(&self, catalog: &str, track_id: u8) -> Result<BackendAudio, BackendError>;

    /// Returns a cover of corrsponding album
    async fn get_cover(&self, catalog: &str) -> Result<Pin<Box<dyn AsyncRead>>, BackendError>;
}

pub enum AnniBackend {
    File(backends::FileBackend),
}

impl AnniBackend {
    pub fn as_backend(&self) -> &impl Backend {
        match self {
            AnniBackend::File(b) => b,
        }
    }

    pub fn as_backend_mut(&mut self) -> &mut impl Backend {
        match self {
            AnniBackend::File(b) => b,
        }
    }
}

#[derive(Debug, Error)]
pub enum BackendError {
    #[error("unknown catalog")]
    UnknownCatalog,

    #[error("invalid path")]
    InvalidPath,

    #[error("file not found")]
    FileNotFound,

    #[error(transparent)]
    IOError(#[from] std::io::Error),

    #[error(transparent)]
    OAuthError(#[from] yup_oauth2::Error),

    #[error(transparent)]
    DriveError(#[from] google_drive3::Error),
}
