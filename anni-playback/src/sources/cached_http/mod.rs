pub mod cache;
pub mod provider;

use std::{
    fs::File,
    io::{ErrorKind, Read, Seek, Write},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Condvar, Mutex,
    },
    thread,
};

use anni_common::models::TrackIdentifier;
use anni_provider::providers::TypedPriorityProvider;
use reqwest::{
    blocking::{Client, Response},
    header::CONTENT_TYPE,
    Url,
};
use thiserror::Error;

use crate::types::MediaSource;
use cache::{CacheAcquire, CacheStore};
use provider::{AudioCodec, AudioQuality, AudioVariant, ProviderProxy};

use super::AnniSource;

const BUF_SIZE: usize = 1024 * 64;

struct DownloadState {
    len: AtomicUsize,
    downloading: AtomicBool,
    error: Mutex<Option<String>>,
    wait_lock: Mutex<()>,
    changed: Condvar,
}

impl DownloadState {
    fn complete(len: usize) -> Self {
        Self {
            len: AtomicUsize::new(len),
            downloading: AtomicBool::new(false),
            error: Mutex::new(None),
            wait_lock: Mutex::new(()),
            changed: Condvar::new(),
        }
    }

    fn downloading() -> Self {
        Self {
            len: AtomicUsize::new(0),
            downloading: AtomicBool::new(true),
            error: Mutex::new(None),
            wait_lock: Mutex::new(()),
            changed: Condvar::new(),
        }
    }

    fn finish(&self, error: Option<String>) {
        let _wait_guard = self.wait_lock.lock().unwrap();
        *self.error.lock().unwrap() = error;
        self.downloading.store(false, Ordering::Release);
        self.changed.notify_all();
    }
}

pub struct CachedHttpSource {
    cache: File,
    state: Arc<DownloadState>,
    pos: usize,
    buffer_signal: Arc<AtomicBool>,
    duration: Option<u64>,
    content_length: Option<u64>,
    cancel_download: Arc<AtomicBool>,
}

impl CachedHttpSource {
    fn logical_len(&self) -> Option<u64> {
        self.content_length.or_else(|| {
            (!self.state.downloading.load(Ordering::Acquire))
                .then(|| self.state.len.load(Ordering::Acquire) as u64)
        })
    }

    fn from_cache(
        cache: File,
        buffer_signal: Arc<AtomicBool>,
        duration: Option<u64>,
    ) -> Result<Self, OpenTrackError> {
        let len = cache.metadata()?.len() as usize;
        buffer_signal.store(false, Ordering::Release);
        Ok(Self {
            cache,
            state: Arc::new(DownloadState::complete(len)),
            pos: 0,
            buffer_signal,
            duration,
            content_length: Some(len as u64),
            cancel_download: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Backwards-compatible constructor using the lossless/original cache variant.
    pub fn new(
        identifier: TrackIdentifier,
        url: impl FnOnce() -> Option<(Url, Option<u64>, Option<u64>)>,
        cache_store: &CacheStore,
        client: Client,
        buffer_signal: Arc<AtomicBool>,
    ) -> Result<Self, OpenTrackError> {
        Self::new_variant(
            identifier,
            AudioVariant::new(AudioQuality::Lossless, AudioCodec::Original),
            url,
            cache_store,
            client,
            buffer_signal,
        )
    }

    pub fn new_variant(
        identifier: TrackIdentifier,
        variant: AudioVariant,
        url: impl FnOnce() -> Option<(Url, Option<u64>, Option<u64>)>,
        cache_store: &CacheStore,
        client: Client,
        buffer_signal: Arc<AtomicBool>,
    ) -> Result<Self, OpenTrackError> {
        if let Some(cache) =
            cache_store.open_variant_if_complete(identifier.inner.copied(), variant)?
        {
            return Self::from_cache(cache, buffer_signal, None);
        }

        let Some((url, duration, content_length)) = url() else {
            return Err(OpenTrackError::NoAvailableAnnil);
        };
        let response = client.get(url).send()?.error_for_status()?;
        let content_length = response.content_length().or(content_length);
        Self::from_response(
            identifier,
            variant,
            response,
            duration,
            content_length,
            cache_store,
            buffer_signal,
        )
    }

    fn from_response(
        identifier: TrackIdentifier,
        variant: AudioVariant,
        response: Response,
        duration: Option<u64>,
        content_length: Option<u64>,
        cache_store: &CacheStore,
        buffer_signal: Arc<AtomicBool>,
    ) -> Result<Self, OpenTrackError> {
        let (reader, mut writer) =
            match cache_store.acquire_variant(identifier.inner.copied(), variant)? {
                CacheAcquire::Hit(cache) => {
                    return Self::from_cache(cache, buffer_signal, duration);
                }
                CacheAcquire::Miss { reader, writer } => (reader, writer),
            };

        let state = Arc::new(DownloadState::downloading());
        let cancel_download = Arc::new(AtomicBool::new(false));
        // This signal means a reader is actually blocked, not merely that a
        // background download is still active.
        buffer_signal.store(false, Ordering::Release);

        thread::Builder::new()
            .name(format!("anni-cache-{}", identifier))
            .spawn({
                let state = Arc::clone(&state);
                let buffer_signal = Arc::clone(&buffer_signal);
                let identifier = identifier.clone();
                let cancel_download = Arc::clone(&cancel_download);
                move || {
                    let result = (|| -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
                        let mut response = response;
                        let mut buffer = [0; BUF_SIZE];
                        loop {
                            if cancel_download.load(Ordering::Acquire) {
                                return Err(std::io::Error::new(
                                    ErrorKind::Interrupted,
                                    "audio download was cancelled",
                                )
                                .into());
                            }
                            match response.read(&mut buffer) {
                                Ok(0) => break,
                                Ok(count) => {
                                    writer.write_all(&buffer[..count])?;
                                    writer.record_downloaded(count);
                                    let _wait_guard = state.wait_lock.lock().unwrap();
                                    state.len.fetch_add(count, Ordering::AcqRel);
                                    state.changed.notify_all();
                                }
                                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                                Err(error) => return Err(error.into()),
                            }
                        }
                        writer.finish(content_length)?;
                        Ok(())
                    })();

                    let error = result.err().map(|error| {
                        log::error!("failed to cache {identifier}: {error}");
                        error.to_string()
                    });
                    buffer_signal.store(false, Ordering::Release);
                    state.finish(error);
                }
            })
            .map_err(OpenTrackError::Io)?;

        Ok(Self {
            cache: reader,
            state,
            pos: 0,
            buffer_signal,
            duration,
            content_length,
            cancel_download,
        })
    }
}

impl Read for CachedHttpSource {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        loop {
            let available = self.state.len.load(Ordering::Acquire);
            if available > self.pos {
                self.buffer_signal.store(false, Ordering::Release);
                let count = <File as Read>::by_ref(&mut self.cache)
                    .take((available - self.pos) as u64)
                    .read(buffer)?;
                self.pos += count;
                return Ok(count);
            }

            if !self.state.downloading.load(Ordering::Acquire) {
                self.buffer_signal.store(false, Ordering::Release);
                if let Some(error) = self.state.error.lock().unwrap().clone() {
                    return Err(std::io::Error::other(error));
                }
                return Ok(0);
            }

            self.buffer_signal.store(true, Ordering::Release);
            let guard = self.state.wait_lock.lock().unwrap();
            let _guard = self
                .state
                .changed
                .wait_while(guard, |_| {
                    self.state.len.load(Ordering::Acquire) <= self.pos
                        && self.state.downloading.load(Ordering::Acquire)
                })
                .unwrap();
        }
    }
}

impl Seek for CachedHttpSource {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        let logical_length = self.logical_len().ok_or_else(|| {
            std::io::Error::new(
                ErrorKind::Unsupported,
                "cannot seek a downloading source with unknown length",
            )
        })?;
        let position = match pos {
            std::io::SeekFrom::Start(position) => i128::from(position),
            std::io::SeekFrom::Current(offset) => self.pos as i128 + i128::from(offset),
            std::io::SeekFrom::End(offset) => i128::from(logical_length) + i128::from(offset),
        };
        let position: u64 = position.try_into().map_err(|_| {
            std::io::Error::new(ErrorKind::InvalidInput, "invalid cache seek position")
        })?;
        if position > logical_length {
            return Err(std::io::Error::new(
                ErrorKind::InvalidInput,
                "cache seek position is past the end of the source",
            ));
        }

        self.cache.seek(std::io::SeekFrom::Start(position))?;
        self.pos = usize::try_from(position).map_err(|_| {
            std::io::Error::new(ErrorKind::InvalidInput, "cache seek position is too large")
        })?;
        Ok(position)
    }
}

impl MediaSource for CachedHttpSource {
    fn is_seekable(&self) -> bool {
        self.state.error.lock().unwrap().is_none() && self.logical_len().is_some()
    }

    fn byte_len(&self) -> Option<u64> {
        self.logical_len()
    }
}

impl AnniSource for CachedHttpSource {
    fn duration_hint(&self) -> Option<u64> {
        self.duration
    }
}

impl Drop for CachedHttpSource {
    fn drop(&mut self) {
        self.cancel_download.store(true, Ordering::Release);
        self.state.changed.notify_all();
    }
}

pub struct CachedAnnilSource {
    source: CachedHttpSource,
    variant: AudioVariant,
}

impl CachedAnnilSource {
    pub fn new(
        track: TrackIdentifier,
        quality: AudioQuality,
        cache_store: &CacheStore,
        client: Client,
        provider: &TypedPriorityProvider<ProviderProxy>,
        buffer_signal: Arc<AtomicBool>,
        opus: bool,
    ) -> Result<Self, OpenTrackError> {
        Self::new_variant(
            track,
            AudioVariant::from_legacy(quality, opus),
            cache_store,
            client,
            provider,
            buffer_signal,
        )
    }

    pub fn new_variant(
        track: TrackIdentifier,
        variant: AudioVariant,
        cache_store: &CacheStore,
        client: Client,
        provider: &TypedPriorityProvider<ProviderProxy>,
        buffer_signal: Arc<AtomicBool>,
    ) -> Result<Self, OpenTrackError> {
        if let Some(cache) = cache_store.open_variant_if_complete(track.inner.copied(), variant)? {
            return Ok(Self {
                source: CachedHttpSource::from_cache(cache, buffer_signal, None)?,
                variant,
            });
        }

        let cloned_track = track.clone();
        let response = provider
            .providers()
            .filter_map(|provider| {
                provider
                    .head_with_client(
                        &client,
                        cloned_track.inner.copied(),
                        variant.quality(),
                        variant.uses_opus(),
                    )
                    .and_then(reqwest::blocking::Response::error_for_status)
                    .inspect_err(|error| log::warn!("annil HEAD failed: {error}"))
                    .ok()
            })
            .next()
            .ok_or(OpenTrackError::NoAvailableAnnil)?;
        let predicted_variant = resolve_variant(
            variant,
            response
                .headers()
                .get("X-Audio-Quality")
                .and_then(|value| value.to_str().ok()),
            response
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
        );
        let duration = response
            .headers()
            .get("X-Duration-Seconds")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse().ok());
        let url = response.url().clone();
        let head_content_length = response.content_length();

        if let Some(cache) =
            cache_store.open_variant_if_complete(track.inner.copied(), predicted_variant)?
        {
            return Ok(Self {
                source: CachedHttpSource::from_cache(cache, buffer_signal, duration)?,
                variant: predicted_variant,
            });
        }

        let response = client.get(url).send()?.error_for_status()?;
        let effective_variant = resolve_variant(
            predicted_variant,
            response
                .headers()
                .get("X-Audio-Quality")
                .and_then(|value| value.to_str().ok()),
            response
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
        );
        let duration = response
            .headers()
            .get("X-Duration-Seconds")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse().ok())
            .or(duration);
        let content_length = response.content_length().or(head_content_length);

        let source = CachedHttpSource::from_response(
            track,
            effective_variant,
            response,
            duration,
            content_length,
            cache_store,
            buffer_signal,
        )?;
        Ok(Self {
            source,
            variant: effective_variant,
        })
    }

    /// The representation actually returned by annil. This may differ from
    /// the request when the server applies guest-quality limits.
    pub fn variant(&self) -> AudioVariant {
        self.variant
    }
}

fn resolve_variant(
    requested: AudioVariant,
    quality_header: Option<&str>,
    content_type: Option<&str>,
) -> AudioVariant {
    let quality = quality_header
        .and_then(|quality| quality.parse().ok())
        .unwrap_or_else(|| requested.quality());
    let content_type = content_type
        .and_then(|value| value.split(';').next())
        .map(str::trim);
    let codec = match content_type {
        Some("audio/ogg" | "audio/opus") => AudioCodec::Opus,
        Some("audio/aac" | "audio/aacp") => AudioCodec::Aac,
        Some(_) => return AudioVariant::new(AudioQuality::Lossless, AudioCodec::Original),
        _ if requested.uses_opus() => AudioCodec::Opus,
        _ => AudioCodec::Aac,
    };
    AudioVariant::new(quality, codec)
}

impl Read for CachedAnnilSource {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        self.source.read(buffer)
    }
}

impl Seek for CachedAnnilSource {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        self.source.seek(pos)
    }
}

impl MediaSource for CachedAnnilSource {
    fn is_seekable(&self) -> bool {
        self.source.is_seekable()
    }

    fn byte_len(&self) -> Option<u64> {
        self.source.byte_len()
    }
}

impl AnniSource for CachedAnnilSource {
    fn duration_hint(&self) -> Option<u64> {
        self.source.duration
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs::{self, File},
        io::{Read, Write},
        net::TcpListener,
        path::PathBuf,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        thread,
        time::{SystemTime, UNIX_EPOCH},
    };

    use anni_common::models::TrackIdentifier;
    use anni_provider::providers::TypedPriorityProvider;
    use reqwest::blocking::Client;

    use crate::types::MediaSource;

    use super::{
        cache::CacheStore, provider::ProviderProxy, resolve_variant, AudioCodec, AudioQuality,
        AudioVariant, CachedAnnilSource, CachedHttpSource, DownloadState,
    };

    fn test_source(
        label: &str,
        content_length: Option<u64>,
        state: Arc<DownloadState>,
    ) -> (CachedHttpSource, PathBuf) {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("anni-playback-cached-source-{label}-{unique}"));
        let cache = File::create(&path).unwrap();
        (
            CachedHttpSource {
                cache,
                state,
                pos: 0,
                buffer_signal: Arc::new(AtomicBool::new(false)),
                duration: None,
                content_length,
                cancel_download: Arc::new(AtomicBool::new(false)),
            },
            path,
        )
    }

    #[test]
    fn server_quality_and_codec_determine_the_cache_variant() {
        let requested = AudioVariant::new(AudioQuality::High, AudioCodec::Opus);
        let resolved = resolve_variant(requested, Some("low"), Some("audio/aac"));
        assert_eq!(resolved.quality(), AudioQuality::Low);
        assert_eq!(resolved.codec(), AudioCodec::Aac);

        let original = resolve_variant(requested, Some("low"), Some("audio/flac"));
        assert_eq!(original.quality(), AudioQuality::Lossless);
        assert_eq!(original.codec(), AudioCodec::Original);
    }

    #[test]
    fn media_source_capabilities_follow_the_available_length() {
        let known_state = Arc::new(DownloadState::downloading());
        let (known, known_path) = test_source("known", Some(12), known_state);
        assert!(known.is_seekable());
        assert_eq!(known.byte_len(), Some(12));
        drop(known);
        fs::remove_file(known_path).unwrap();

        let unknown_state = Arc::new(DownloadState::downloading());
        unknown_state.len.store(12, Ordering::Release);
        let (unknown, unknown_path) = test_source("unknown", None, Arc::clone(&unknown_state));
        assert!(!unknown.is_seekable());
        assert_eq!(unknown.byte_len(), None);

        unknown_state.finish(None);
        assert!(unknown.is_seekable());
        assert_eq!(unknown.byte_len(), Some(12));
        drop(unknown);
        fs::remove_file(unknown_path).unwrap();

        let failed_state = Arc::new(DownloadState::downloading());
        failed_state.finish(Some("download failed".into()));
        let (failed, failed_path) = test_source("failed", Some(12), failed_state);
        assert!(!failed.is_seekable());
        drop(failed);
        fs::remove_file(failed_path).unwrap();
    }

    #[test]
    fn get_representation_wins_when_head_predicts_a_transcode() {
        let audio = include_bytes!("../../../../assets/1s.flac").to_vec();
        let listener = match TcpListener::bind("127.0.0.1:0") {
            Ok(listener) => listener,
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                eprintln!("skipping loopback HTTP test: {error}");
                return;
            }
            Err(error) => panic!("could not bind loopback test server: {error}"),
        };
        let address = listener.local_addr().unwrap();
        let server_audio = audio.clone();
        let server = thread::spawn(move || {
            for expected_method in ["HEAD", "GET"] {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = Vec::new();
                let mut chunk = [0; 1024];
                while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                    let count = stream.read(&mut chunk).unwrap();
                    assert!(count > 0);
                    request.extend_from_slice(&chunk[..count]);
                }
                let request = String::from_utf8(request).unwrap();
                assert!(request.starts_with(expected_method));

                let content_type = if expected_method == "HEAD" {
                    "audio/aac"
                } else {
                    "audio/flac"
                };
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nX-Audio-Quality: low\r\nX-Duration-Seconds: 1\r\n\r\n",
                    server_audio.len()
                )
                .unwrap();
                if expected_method == "GET" {
                    stream.write_all(&server_audio).unwrap();
                }
            }
        });

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let cache_path = std::env::temp_dir().join(format!("anni-playback-http-{unique}"));
        let cache = CacheStore::new(cache_path.clone());
        let client = Client::builder().no_proxy().build().unwrap();
        let provider = TypedPriorityProvider::new(vec![(
            0,
            ProviderProxy::new(format!("http://{address}"), "token".into(), client.clone()),
        )]);
        let track: TrackIdentifier = "65cf12dc-9717-4503-9901-848e8cd3ebff/1/1".parse().unwrap();
        let requested = AudioVariant::new(AudioQuality::Low, AudioCodec::Aac);
        let mut source = CachedAnnilSource::new_variant(
            track.clone(),
            requested,
            &cache,
            client,
            &provider,
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap();

        assert_eq!(
            source.variant(),
            AudioVariant::new(AudioQuality::Lossless, AudioCodec::Original)
        );
        let mut downloaded = Vec::new();
        source.read_to_end(&mut downloaded).unwrap();
        assert_eq!(downloaded, audio);
        assert!(cache
            .location_of_variant(
                track.inner.copied(),
                AudioVariant::new(AudioQuality::Lossless, AudioCodec::Original),
            )
            .exists());
        assert!(!cache
            .location_of_variant(track.inner.copied(), requested)
            .exists());

        server.join().unwrap();
        fs::remove_dir_all(cache_path).unwrap();
    }
}

#[non_exhaustive]
#[derive(Debug, Error)]
pub enum OpenTrackError {
    #[error("No available annil")]
    NoAvailableAnnil,
    #[error("Io Error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Http Error: {0}")]
    Http(#[from] reqwest::Error),
}
