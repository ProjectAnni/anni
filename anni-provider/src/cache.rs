use crate::{AnniProvider, AudioInfo, AudioResourceReader, ProviderError, Range, ResourceReader};
use anni_common::models::{RawTrackIdentifier, TrackIdentifier};
use async_trait::async_trait;
use dashmap::DashMap;
use lru::LruCache;
use parking_lot::RwLock;
use std::borrow::{Borrow, Cow};
use std::collections::HashSet;
use std::future::Future;
use std::num::NonZeroU8;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::fs::File;
use tokio::io::{AsyncRead, AsyncReadExt, ReadBuf};
use tokio::sync::Mutex;
use tokio::time::Duration;

pub struct CacheProvider<T>
where
    T: AnniProvider + Send,
{
    inner: T,
    pool: Arc<CachePool>,
}

impl<T> CacheProvider<T>
where
    T: AnniProvider + Send,
{
    pub fn new(inner: T, pool: Arc<CachePool>) -> Self {
        Self { inner, pool }
    }

    pub async fn invalidate(&self, album_id: &str, disc_id: NonZeroU8, track_id: NonZeroU8) {
        let key = RawTrackIdentifier::new(album_id, disc_id, track_id);
        self.pool.remove(&key).await;
    }
}

#[async_trait]
impl<T> AnniProvider for CacheProvider<T>
where
    T: AnniProvider + Send,
{
    async fn albums(&self) -> Result<HashSet<Cow<str>>, ProviderError> {
        self.inner.albums().await
    }

    async fn get_audio_info(
        &self,
        album_id: &str,
        disc_id: NonZeroU8,
        track_id: NonZeroU8,
    ) -> Result<AudioInfo, ProviderError> {
        if let Some(info) = self
            .pool
            .get_cached_audio_info(album_id, disc_id, track_id)
            .await
        {
            Ok(info)
        } else {
            self.inner.get_audio_info(album_id, disc_id, track_id).await
        }
    }

    async fn get_audio(
        &self,
        album_id: &str,
        disc_id: NonZeroU8,
        track_id: NonZeroU8,
        range: Range,
    ) -> Result<AudioResourceReader, ProviderError> {
        self.pool
            .fetch_audio(
                album_id,
                disc_id,
                track_id,
                range,
                self.inner.get_audio(
                    album_id,
                    disc_id,
                    track_id,
                    Range::FULL, // cache does not pass range to the underlying provider
                ),
            )
            .await
    }

    async fn get_cover(
        &self,
        album_id: &str,
        disc_id: Option<NonZeroU8>,
    ) -> Result<ResourceReader, ProviderError> {
        // TODO: cache cover
        self.inner.get_cover(album_id, disc_id).await
    }

    async fn reload(&mut self) -> Result<(), ProviderError> {
        // reload the inner provider
        self.inner.reload().await
    }
}

pub struct CachePool {
    /// Root of cache folder
    root: PathBuf,
    /// Maximum space used by cache
    max_size: Option<usize>,
    cache: DashMap<TrackIdentifier, Arc<CacheItem>>,
    // https://github.com/xacrimon/dashmap/issues/189
    // TODO: Use LFU instead of LRU
    last_used: Mutex<LruCache<TrackIdentifier, Arc<Mutex<u8>>>>,
}

impl CachePool {
    pub fn new<P>(root: P, max_size: Option<usize>) -> Self
    where
        P: AsRef<Path>,
    {
        Self {
            root: PathBuf::from(root.as_ref()),
            max_size,
            cache: Default::default(),
            last_used: Mutex::new(LruCache::unbounded()),
        }
    }

    async fn fetch_audio(
        &self,
        album_id: &str,
        disc_id: NonZeroU8,
        track_id: NonZeroU8,
        range: Range,
        on_miss: impl Future<Output = Result<AudioResourceReader, ProviderError>>,
    ) -> Result<AudioResourceReader, ProviderError> {
        let key = RawTrackIdentifier::new(album_id, disc_id, track_id);
        let item = if !self.has_cache(album_id, disc_id, track_id).await {
            // on miss, set state to cached first
            let mutex = Arc::new(Mutex::new(0));
            let handle = mutex.clone().lock_owned().await;
            self.last_used.lock().await.put(key.to_owned(), mutex);

            // get data, return directly if it's a partial request
            let result = on_miss.await?;

            // prepare for new item
            let path = self.root.join(key.album_id.as_ref()).join(format!(
                "{}_{}",
                key.disc_id.get(),
                key.track_id.get()
            ));
            let mut file = File::create(&path).await?;

            let AudioResourceReader {
                info, mut reader, ..
            } = result;
            let item = Arc::new(CacheItem::new(path, info, false));

            // remove old item if space is full
            if let Some(max_size) = self.max_size {
                if self.space_used() > max_size {
                    // get the first item of BTreeMap
                    let mut write = self.last_used.lock().await;
                    let key = write.pop_lru().unwrap();
                    // remove it from cache map
                    // drop would do the removal
                    self.remove(&key.0.borrow()).await;
                }
            }

            // write to map
            self.cache.insert(key.to_owned(), item.clone());
            // item is set to cached, release lock
            drop(handle);

            // cache
            let item_spawn = item.clone();
            tokio::spawn(async move {
                let actual_size = tokio::io::copy(&mut reader, &mut file).await.unwrap() as usize;
                if item_spawn.size() != actual_size {
                    // TODO: should not happen, throw error here
                    item_spawn.set_size(actual_size);
                }
                item_spawn.set_cached(true);
            });
            item
        } else {
            // resource requested, but not added to cache map yet
            if !self.cache.contains_key(&key) {
                // await cache mutex
                let mutex = {
                    let mut map = self.last_used.lock().await;
                    map.get(&key).unwrap().clone()
                };
                let _ = mutex.lock().await;
            }
            // update last_used time
            self.last_used.lock().await.get(&key).unwrap();
            self.cache.get(&key).unwrap().clone()
        };

        Ok(item
            .to_audio_resource_reader(File::open(&item.path).await?, range)
            .await)
    }

    async fn remove<'a>(&self, key: &RawTrackIdentifier<'a>) {
        self.cache.remove(key).map(|r| r.1.set_cached(false));
        self.last_used.lock().await.pop(key);
    }

    async fn get_cached_audio_info(
        &self,
        album_id: &str,
        disc_id: NonZeroU8,
        track_id: NonZeroU8,
    ) -> Option<AudioInfo> {
        self.cache
            .get(&RawTrackIdentifier::new(album_id, disc_id, track_id))
            .and_then(|r| {
                if *r.cached.read() {
                    Some(AudioInfo {
                        extension: r.ext.clone(),
                        size: *r.size.read(),
                        duration: r.duration,
                    })
                } else {
                    None
                }
            })
    }

    async fn has_cache(&self, album_id: &str, disc_id: NonZeroU8, track_id: NonZeroU8) -> bool {
        self.last_used
            .lock()
            .await
            .contains(&RawTrackIdentifier::new(album_id, disc_id, track_id))
    }

    fn space_used(&self) -> usize {
        self.cache
            .iter()
            .map(|i| i.size())
            .reduce(|a, b| a + b)
            .unwrap_or(0)
    }
}

struct CacheItem {
    ext: String,
    path: PathBuf,
    size: RwLock<usize>,
    duration: u64,
    cached: RwLock<bool>,
}

impl CacheItem {
    fn new(path: PathBuf, info: AudioInfo, cached: bool) -> Self {
        let AudioInfo {
            extension: ext,
            duration,
            size,
        } = info;
        CacheItem {
            path,
            ext,
            size: RwLock::new(size),
            duration,
            cached: RwLock::new(cached),
        }
    }

    fn size(&self) -> usize {
        *self.size.read()
    }

    fn set_size(&self, size: usize) {
        *self.size.write() = size;
    }

    fn cached(&self) -> bool {
        *self.cached.read()
    }

    fn set_cached(&self, cached: bool) {
        *self.cached.write() = cached
    }
}

#[async_trait::async_trait]
trait CacheReader {
    fn to_reader(&self, file: File) -> CacheItemReader;

    async fn to_audio_resource_reader(&self, file: File, range: Range) -> AudioResourceReader;
}

#[async_trait::async_trait]
impl CacheReader for Arc<CacheItem> {
    fn to_reader(&self, file: File) -> CacheItemReader {
        CacheItemReader {
            item: self.clone(),
            file: Box::pin(file),
            filled: 0,
            timer: None,
        }
    }

    async fn to_audio_resource_reader(&self, file: File, range: Range) -> AudioResourceReader {
        let mut reader = self.to_reader(file);
        if range.start > 0 {
            let reader = &mut reader;
            let _ = tokio::io::copy(&mut reader.take(range.start), &mut tokio::io::sink()).await;
        }
        let length = range.length();
        let reader: ResourceReader = match length {
            Some(length) => Box::pin(reader.take(length)),
            None => Box::pin(reader),
        };

        AudioResourceReader {
            info: AudioInfo {
                extension: self.ext.clone(),
                size: self.size(),
                duration: self.duration,
            },
            range,
            reader,
        }
    }
}

impl Drop for CacheItem {
    fn drop(&mut self) {
        // not cached, means:
        // a. file not fully cached and program reaches program termination
        // b. manually set cached to false
        if !self.cached() {
            if let Err(e) = std::fs::remove_file(&self.path) {
                log::error!("Failed to drop CacheItem: {}", e);
            }
        }
    }
}

struct CacheItemReader {
    item: Arc<CacheItem>,
    file: Pin<Box<File>>,
    filled: usize,

    timer: Option<Pin<Box<dyn Future<Output = ()> + Send>>>,
}

impl AsyncRead for CacheItemReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        // Wait mode
        if self.timer.is_some() {
            let task = self.timer.as_mut().unwrap();
            // poll the saved timer
            let result = task.as_mut().poll(cx);
            match result {
                // timer ready, stop waiting
                Poll::Ready(_) => self.timer = None,
                // timer pending, wait
                Poll::Pending => return Poll::Pending,
            }
        }

        // Read mode
        // save filled buf length before poll_read
        let before = buf.filled().len();
        let result = self.file.as_mut().poll_read(cx, buf);
        match result {
            Poll::Ready(result) => {
                match result {
                    Ok(_) => {
                        let now = buf.filled().len();
                        if before != now {
                            self.filled += now - before;
                            Poll::Ready(Ok(()))
                        } else if self.item.cached() {
                            if self.filled != self.item.size() {
                                // caching finished just now
                                // wake immediately to finish the last part
                                cx.waker().wake_by_ref();
                                Poll::Pending
                            } else {
                                // EOF
                                Poll::Ready(Ok(()))
                            }
                        } else {
                            // not done, wait for more data
                            // set up timer to wait
                            self.timer =
                                Some(Box::pin(tokio::time::sleep(Duration::from_millis(100))));
                            // wait immediately to poll the timer
                            cx.waker().wake_by_ref();
                            Poll::Pending
                        }
                    }
                    // poll error
                    Err(e) => Poll::Ready(Err(e)),
                }
            }
            // wait
            Poll::Pending => Poll::Pending,
        }
    }
}
