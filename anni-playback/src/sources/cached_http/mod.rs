pub mod cache;
pub mod provider;

use std::{
    fs::File,
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
    pos: Arc<AtomicUsize>,
    is_buffering: Arc<AtomicBool>,
    #[allow(unused)]
    buffer_signal: Arc<AtomicBool>,
    duration: Option<u64>,
}

impl CachedHttpSource {
    /// `cache_path` is the path to cache file.
    pub fn new(
        identifier: TrackIdentifier,
        url: impl FnOnce() -> Option<(Url, Option<u64>)>,
        cache_store: &CacheStore,
        client: Client,
        buffer_signal: Arc<AtomicBool>,
    ) -> Result<Self, OpenTrackError> {
        let cache = match cache_store.acquire(identifier.inner.copied())? {
            Ok(cache) => {
                let buf_len = cache.metadata()?.len() as usize;

                return Ok(Self {
                    identifier,
                    cache,
                    buf_len: Arc::new(AtomicUsize::new(buf_len)),
                    pos: Arc::new(AtomicUsize::new(0)),
                    is_buffering: Arc::new(AtomicBool::new(false)),
                    buffer_signal,
                    duration: None,
                });
            }
            Err(cache) => cache,
        };

        let buf_len = Arc::new(AtomicUsize::new(0));
        let is_buffering = Arc::new(AtomicBool::new(true));
        let pos = Arc::new(AtomicUsize::new(0));

        let (url, duration) = url().ok_or(OpenTrackError::NoAvailableAnnil)?;

        log::debug!("got duration {duration:?}");

        thread::spawn({
            let mut cache = cache.try_clone()?;
            let buf_len = Arc::clone(&buf_len);
            let pos = Arc::clone(&pos);
            let mut buf = [0; BUF_SIZE];
            let is_buffering = Arc::clone(&is_buffering);
            let identifier = identifier.clone();

            move || {
                let mut response = match client.get(url).send() {
                    Ok(r) => r,
                    Err(e) => {
                        log::error!("failed to send request: {e}");
                        return;
                    }
                };

                loop {
                    match response.read(&mut buf) {
                        Ok(0) => {
                            is_buffering.store(false, Ordering::Release);
                            log::info!("{identifier} reached eof");
                            break;
                        }
                        Ok(n) => {
                            let pos = pos.load(Ordering::Acquire);
                            if let Err(e) = cache.write_all(&buf[..n]) {
                                log::error!("{e}")
                            }

                            log::trace!("wrote {n} bytes to {identifier}");

                            let _ = cache.seek(std::io::SeekFrom::Start(pos as u64));
                            let _ = cache.flush();

                            buf_len.fetch_add(n, Ordering::AcqRel);
                        }
                        Err(e) if e.kind() == ErrorKind::Interrupted => {}
                        Err(e) => {
                            log::error!("{e}");
                            is_buffering.store(false, Ordering::Release);
                        }
                    }
                }
            }
        });

        Ok(Self {
            identifier,
            cache,
            buf_len,
            pos,
            is_buffering,
            buffer_signal,
            duration,
        })
    }
}

impl Read for CachedHttpSource {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // let n = self.cache.read(buf)?;
        // self.pos.fetch_add(n, Ordering::AcqRel);
        // log::trace!("read {n} bytes");
        // Ok(n)

        loop {
            let has_buf = self.buf_len.load(Ordering::Acquire) > self.pos.load(Ordering::Acquire);
            let is_buffering = self.is_buffering.load(Ordering::Acquire);

            if has_buf {
                let n = self.cache.read(buf)?;
                log::trace!("read {n} bytes from {}", self.identifier);
                if n == 0 {
                    continue;
                }
                self.pos.fetch_add(n, Ordering::AcqRel);
                break Ok(n);
            } else if !is_buffering {
                break Ok(0);
            }
        }
    }
}

impl Seek for CachedHttpSource {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        let p = self.cache.seek(pos)?;
        self.pos.store(p as usize, Ordering::Release);
        Ok(p)
    }
}

impl MediaSource for CachedHttpSource {
    fn is_seekable(&self) -> bool {
        !self.is_buffering.load(Ordering::Relaxed)
    }

    fn byte_len(&self) -> Option<u64> {
        let len = self.buf_len.load(Ordering::Acquire) as u64;
        log::trace!("returning buf_len {len}");
        Some(len)
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
            .map(|response| {
                let duration = response
                    .headers()
                    .get("X-Duration-Seconds")
                    .and_then(|v| v.to_str().ok().and_then(|dur| dur.parse().ok()));
                (response.url().clone(), duration)
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
