use async_trait::async_trait;
use tokio::io::AsyncRead;
use regex::Regex;
use std::pin::Pin;
use thiserror::Error;
use crate::backends;
use std::borrow::Cow;

/// Backend is a common trait for anni backends.
/// It provides functions to update albums, and read from an initialized backend.
#[async_trait]
pub trait Backend {
    /// Cache indicator for remote file systems.
    fn need_cache(&self) -> bool;

    /// Whether backend has an album.
    async fn has(&self, catalog: &str) -> bool;

    /// Get catalog of albums available.
    async fn albums(&self) -> Vec<Cow<str>>;

    /// Update album information provided by backend.
    /// Backends usually need to save a map between catalog and path, so this method is &mut.
    async fn update_albums(&mut self) -> Result<(), BackendError>;

    /// Returns a reader implements AsyncRead for content reading
    async fn get_audio(&self, catalog: &str, track_id: u8) -> Result<Pin<Box<dyn AsyncRead>>, BackendError>;

    /// Returns a cover of corrsponding album
    async fn get_cover(&self, catalog: &str) -> Result<Pin<Box<dyn AsyncRead>>, BackendError>;
}

lazy_static::lazy_static! {
    static ref ALBUM_REGEX: Regex = Regex::new(r"^\[(?:\d{2}|\d{4})-?\d{2}-?\d{2}]\[([^]]+)] .+$").unwrap();
    static ref DISC_REGEX: Regex = Regex::new(r"^\[([^]]+)] .+ \[Disc \d+]$").unwrap();
}

pub(crate) fn extract_album<S: AsRef<str>>(name: S) -> Option<String> {
    ALBUM_REGEX.captures(name.as_ref()).map(|r| r.get(1).unwrap().as_str().to_owned())
}

pub(crate) fn extract_disc<S: AsRef<str>>(name: S) -> Option<String> {
    DISC_REGEX.captures(name.as_ref()).map(|r| r.get(1).unwrap().as_str().to_owned())
}

pub enum AnniBackend {
    File(backends::FileBackend),
    StrictFile(backends::StrictFileBackend),
}

impl AnniBackend {
    pub fn as_backend(&self) -> Box<&(dyn Backend + Send)> {
        match self {
            AnniBackend::File(b) => Box::new(b),
            AnniBackend::StrictFile(b) => Box::new(b),
        }
    }
    pub fn as_backend_mut(&mut self) -> Box<&mut (dyn Backend + Send)> {
        match self {
            AnniBackend::File(b) => Box::new(b),
            AnniBackend::StrictFile(b) => Box::new(b),
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
}

#[test]
fn test_extract_catalog() {
    assert_eq!(extract_album("[210306][CATA-LOG] Title"), Some("CATA-LOG".to_owned()));
    assert_eq!(extract_album("233"), None);
}