use async_trait::async_trait;
use std::borrow::Cow;
use std::collections::HashSet;
use std::num::NonZeroU8;
use std::path::PathBuf;
use std::pin::Pin;
use thiserror::Error;
use tokio::io::AsyncRead;
use tokio_stream::Stream;

pub type Result<T> = std::result::Result<T, ProviderError>;
pub type ResourceReader = Pin<Box<dyn AsyncRead + Send>>;

#[derive(Clone)]
pub struct AudioInfo {
    /// File extension of the file
    pub extension: String,
    /// File size of the file
    pub size: usize,
    /// Audio duration of the file, in milliseconds
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

#[derive(Clone, Copy)]
pub struct Range {
    pub start: u64,
    pub end: Option<u64>,
    pub total: Option<u64>,
}

impl Range {
    pub const FULL: Range = Range {
        start: 0,
        end: None,
        total: None,
    };

    pub const FLAC_HEADER: Range = Range {
        start: 0,
        end: Some(42),
        total: None,
    };

    /// create a new range with given start and end offset
    pub fn new(start: u64, end: Option<u64>) -> Self {
        Self {
            start,
            end,
            total: None,
        }
    }

    /// get the length of the range
    /// if the range is full, returns None
    pub fn length(&self) -> Option<u64> {
        self.end.map(|end| end - self.start + 1)
    }

    /// return length limited by a limit(usually actual file size)
    pub fn length_limit(&self, limit: u64) -> u64 {
        let end = match self.end {
            Some(end) => std::cmp::min(end, limit),
            None => limit,
        };
        end - self.start + 1
    }

    /// return a new Range with updated end property
    pub fn end_with(&self, end: u64) -> Self {
        Self {
            start: self.start,
            end: match self.end {
                Some(e) => Some(e.min(end - 1)),
                None => Some(end - 1),
            },
            total: match self.total {
                Some(total) => Some(total.min(end)),
                None => Some(end),
            },
        }
    }

    pub fn is_full(&self) -> bool {
        self.start == 0 && self.end.is_none()
    }

    pub fn contains_flac_header(&self) -> bool {
        if self.start == 0 {
            match self.end {
                Some(end) => end + 1 >= 42,
                None => true,
            }
        } else {
            false
        }
    }

    pub fn to_range_header(&self) -> Option<String> {
        if self.is_full() {
            None
        } else {
            Some(match self.end {
                Some(end) => format!("bytes={}-{}", self.start, end),
                None => format!("bytes={}-", self.start),
            })
        }
    }

    pub fn to_content_range_header(&self) -> String {
        if self.is_full() {
            "bytes */*".to_string()
        } else {
            match (self.end, self.total) {
                (Some(end), Some(total)) => format!("bytes {}-{}/{}", self.start, end, total),
                (Some(end), None) => format!("bytes {}-{}/*", self.start, end),
                _ => format!("bytes {}-", self.start),
            }
        }
    }
}

/// AnniProvider is a common trait for anni resource providers.
/// It provides functions to get cover, audio, album list and reload.
#[async_trait]
// work around to add a default implementation for has_albums()
// https://github.com/rust-lang/rust/issues/51443
// https://docs.rs/async-trait/latest/async_trait/index.html#dyn-traits
pub trait AnniProvider: Sync {
    /// Get album information provided by provider.
    async fn albums(&self) -> Result<HashSet<Cow<str>>>;

    /// Returns whether given album exists
    async fn has_album(&self, album_id: &str) -> bool {
        self.albums()
            .await
            .unwrap_or(HashSet::new())
            .contains(album_id)
    }

    /// Get audio info describing basic information of the audio file.
    async fn get_audio_info(
        &self,
        album_id: &str,
        disc_id: NonZeroU8,
        track_id: NonZeroU8,
    ) -> Result<AudioInfo> {
        Ok(self
            .get_audio(album_id, disc_id, track_id, Range::FLAC_HEADER)
            .await?
            .info)
    }

    /// Returns a reader implements AsyncRead for content reading
    async fn get_audio(
        &self,
        album_id: &str,
        disc_id: NonZeroU8,
        track_id: NonZeroU8,
        range: Range,
    ) -> Result<AudioResourceReader>;

    /// Returns a cover of corresponding album
    async fn get_cover(&self, album_id: &str, disc_id: Option<NonZeroU8>)
        -> Result<ResourceReader>;

    /// Reloads the provider for new albums
    async fn reload(&mut self) -> Result<()>;
}

#[async_trait]
impl AnniProvider for Box<dyn AnniProvider + Send + Sync> {
    async fn albums(&self) -> Result<HashSet<Cow<str>>> {
        self.albums().await
    }

    async fn has_album(&self, album_id: &str) -> bool {
        self.has_album(album_id).await
    }

    async fn get_audio_info(
        &self,
        album_id: &str,
        disc_id: NonZeroU8,
        track_id: NonZeroU8,
    ) -> Result<AudioInfo> {
        self.get_audio_info(album_id, disc_id, track_id).await
    }

    async fn get_cover(
        &self,
        album_id: &str,
        disc_id: Option<NonZeroU8>,
    ) -> Result<ResourceReader> {
        self.get_cover(album_id, disc_id).await
    }

    async fn get_audio(
        &self,
        album_id: &str,
        disc_id: NonZeroU8,
        track_id: NonZeroU8,
        range: Range,
    ) -> Result<AudioResourceReader> {
        self.get_audio(album_id, disc_id, track_id, range).await
    }

    async fn reload(&mut self) -> Result<()> {
        self.reload().await
    }
}

#[derive(Clone)]
pub struct FileEntry {
    pub name: String,
    pub path: PathBuf,
}

#[async_trait]
pub trait FileSystemProvider: Sync {
    /// List sub folders
    async fn children(
        &self,
        path: &PathBuf,
    ) -> Result<Pin<Box<dyn Stream<Item = FileEntry> + Send>>>;

    /// Get file entry in a folder with given prefix
    async fn get_file_entry_by_prefix(&self, parent: &PathBuf, prefix: &str) -> Result<FileEntry>;

    /// Get file reader
    async fn get_file(&self, path: &PathBuf, range: Range) -> Result<ResourceReader>;

    /// Get audio info: (extension ,size)
    async fn get_audio_info(&self, path: &PathBuf) -> Result<(String, usize)>;

    // TODO: move this method to a sub trait
    async fn get_audio_file(&self, path: &PathBuf, range: Range) -> Result<AudioResourceReader> {
        let reader = self.get_file(path, range).await?;
        let metadata = self.get_audio_info(path).await?;
        let (duration, reader) = crate::utils::read_duration(reader, range).await?;
        Ok(AudioResourceReader {
            info: AudioInfo {
                extension: metadata.0,
                size: metadata.1,
                duration,
            },
            range: Range {
                start: range.start,
                end: Some(range.end.unwrap_or(metadata.1 as u64 - 1)),
                total: Some(metadata.1 as u64),
            },
            reader,
        })
    }

    /// Reload
    async fn reload(&mut self) -> Result<()>;
}

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("invalid path")]
    InvalidPath,

    #[error("file not found")]
    FileNotFound,

    #[error(transparent)]
    IOError(#[from] std::io::Error),

    #[cfg(feature = "repo")]
    #[error(transparent)]
    RepoError(#[from] anni_repo::error::Error),

    #[cfg(feature = "anni-google-drive3")]
    #[error(transparent)]
    OAuthError(#[from] anni_google_drive3::oauth2::Error),

    #[cfg(feature = "anni-google-drive3")]
    #[error(transparent)]
    DriveError(#[from] anni_google_drive3::Error),

    #[cfg(feature = "reqwest")]
    #[error(transparent)]
    RequestError(#[from] reqwest::Error),

    #[error(transparent)]
    FlacError(#[from] anni_flac::error::FlacError),

    #[error("an error occurred")]
    GeneralError,
}

pub fn strict_album_path(root: &PathBuf, album_id: &str, layer: usize) -> PathBuf {
    let mut res = root.clone();
    for i in 0..layer {
        res.push(match &album_id[i * 2..=i * 2 + 1].trim_start_matches('0') {
            &"" => "0",
            s @ _ => s,
        });
    }
    res.join(album_id)
}

pub(crate) fn content_range_to_range(content_range: Option<&str>) -> Range {
    match content_range {
        Some(content_range) => {
            // if content range header is invalid, return the full range
            if content_range.len() <= 6 {
                return Range::FULL;
            }

            // else, parse the range
            // Content-Range: bytes 0-1023/10240
            //                      | offset = 6
            let content_range = &content_range[6..];
            let (from, content_range) =
                content_range.split_once('-').unwrap_or((content_range, ""));
            let (to, total) = content_range.split_once('/').unwrap_or((content_range, ""));

            Range {
                start: from.parse().unwrap_or(0),
                end: to.parse().ok(),
                total: total.parse().ok(),
            }
        }
        None => Range::FULL,
    }
}
