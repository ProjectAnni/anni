pub mod cache;
pub mod provider;

use std::{
    fs::File,
    hint::spin_loop,
    io::{ErrorKind, Read, Seek, Write},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    thread,
};

use anni_common::models::TrackIdentifier;
use anni_provider::providers::TypedPriorityProvider;
use reqwest::{blocking::Client, Url};
use thiserror::Error;

use crate::types::MediaSource;
use provider::{AudioQuality, ProviderProxy};

use cache::CacheStore;

use super::AnniSource;

const BUF_SIZE: usize = 1024 * 64; // 64k

pub struct CachedHttpSource {
    identifier: TrackIdentifier,
    cache: File,
    buf_len: Arc<AtomicUsize>,
    pos: usize,
    is_buffering: Arc<AtomicBool>,
    #[allow(unused)]
    buffer_signal: Arc<AtomicBool>,
    duration: Option<u64>,
    content_length: Option<u64>,
}

impl CachedHttpSource {
    /// `cache_path` is the path to cache file.
    pub fn new(
        identifier: TrackIdentifier,
        url: impl FnOnce() -> Option<(Url, Option<u64>, Option<u64>)>,
        cache_store: &CacheStore,
        client: Client,
        buffer_signal: Arc<AtomicBool>,
    ) -> Result<Self, OpenTrackError> {
        let (reader, writer) = match cache_store.acquire(identifier.inner.copied())? {
            Ok(cache) => {
                let buf_len = cache.metadata()?.len() as usize;

                return Ok(Self {
                    identifier,
                    cache,
                    buf_len: Arc::new(AtomicUsize::new(buf_len)),
                    pos: 0,
                    is_buffering: Arc::new(AtomicBool::new(false)),
                    buffer_signal,
                    duration: None,
                    content_length: Some(buf_len as u64),
                });
            }
            Err(cache) => cache,
        };

        let buf_len = Arc::new(AtomicUsize::new(0));
        let is_buffering = Arc::new(AtomicBool::new(true));

        let (url, duration, content_length) = url().ok_or(OpenTrackError::NoAvailableAnnil)?;

        log::debug!("got duration {duration:?}");

        thread::spawn({
            let mut cache = writer;
            let buf_len = Arc::clone(&buf_len);
            let mut buf = [0; BUF_SIZE];
            let is_buffering = Arc::clone(&is_buffering);
            let identifier = identifier.clone();

            move || {
                let mut response = match client.get(url).send() {
                    Ok(r) => r,
                    Err(e) => {
                        log::error!("failed to send request: {e}");
                        is_buffering.store(false, Ordering::Release);
                        return;
                    }
                };

                loop {
                    match response.read(&mut buf) {
                        Ok(0) => {
                            log::info!("{identifier} reached eof");
                            break;
                        }
                        Ok(n) => {
                            if let Err(e) = cache.write_all(&buf[..n]) {
                                log::error!("{e}");
                                break;
                            }

                            let _ = cache.flush();
                            buf_len.fetch_add(n, Ordering::AcqRel);

                            log::trace!("wrote {n} bytes to {identifier}");
                        }
                        Err(e) if e.kind() == ErrorKind::Interrupted => {}
                        Err(e) => {
                            log::error!("{e}");
                            break;
                        }
                    }
                }

                is_buffering.store(false, Ordering::Release);
            }
        });

        Ok(Self {
            identifier,
            cache: reader,
            buf_len,
            pos: 0,
            is_buffering,
            buffer_signal,
            duration,
            content_length,
        })
    }
}

impl Read for CachedHttpSource {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // A naive spin loop that waits until we have more data to read.
        loop {
            let is_buffering = self.is_buffering.load(Ordering::Acquire);
            let buf_len = self.buf_len.load(Ordering::Acquire);
            let has_buf = buf_len > self.pos;

            if has_buf {
                let n = <File as Read>::by_ref(&mut self.cache)
                    .take((buf_len - self.pos) as u64)
                    .read(buf)?; // ensure not exceeding the buffer

                log::trace!("read {n} bytes from {}", self.identifier);

                self.pos += n;
                break Ok(n);
            } else if !is_buffering {
                break Ok(0);
            } else {
                spin_loop();
            }
        }
    }
}

impl Seek for CachedHttpSource {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        let p = self.cache.seek(pos)?;
        self.pos = p as usize;
        Ok(p)
    }
}

impl MediaSource for CachedHttpSource {
    fn is_seekable(&self) -> bool {
        !self.is_buffering.load(Ordering::Acquire)
    }

    fn byte_len(&self) -> Option<u64> {
        log::trace!("returning byte len {:?}", self.content_length);
        self.content_length
    }
}

impl AnniSource for CachedHttpSource {
    fn duration_hint(&self) -> Option<u64> {
        self.duration
    }
}

pub struct CachedAnnilSource(CachedHttpSource);

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
        let cloned_track = track.clone();

        let mut source = provider
            .providers()
            .filter_map(|p| {
                p.head(cloned_track.inner.copied(), quality, opus)
                    .and_then(|r| r.error_for_status())
                    .inspect_err(|e| log::warn!("{e}"))
                    .ok()
            })
            .map(|r| {
                let (url, headers) = (r.url(), r.headers());
                let parse_header = |key| headers.get(key).and_then(|v| v.to_str().ok());
                let duration = parse_header("X-Duration-Seconds").and_then(|v| v.parse().ok());
                if let Some(content_length) = r.content_length() {
                    let _ = cache_store.store_info(
                        cloned_track.inner.copied(),
                        "content-length",
                        content_length,
                    );
                }
                (url.clone(), duration, r.content_length())
            });

        CachedHttpSource::new(track, || source.next(), cache_store, client, buffer_signal).map(Self)
    }
}

impl Read for CachedAnnilSource {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}

impl Seek for CachedAnnilSource {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        self.0.seek(pos)
    }
}

impl MediaSource for CachedAnnilSource {
    fn is_seekable(&self) -> bool {
        self.0.is_seekable()
    }

    fn byte_len(&self) -> Option<u64> {
        self.0.byte_len()
    }
}

impl AnniSource for CachedAnnilSource {
    fn duration_hint(&self) -> Option<u64> {
        self.0.duration
    }
}

#[derive(Debug, Error)]
pub enum OpenTrackError {
    #[error("No available annil")]
    NoAvailableAnnil,
    #[error("Io Error: {0}")]
    Io(#[from] std::io::Error),
}
