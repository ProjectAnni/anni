use crate::{Backend, BackendError, BackendReaderExt, BackendReader};
use std::path::{PathBuf, Path};
use std::collections::{HashSet, HashMap, BTreeMap};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::io::{AsyncRead, ReadBuf};
use std::task::{Context, Poll};
use std::pin::Pin;
use tokio::time::Duration;
use std::future::Future;
use tokio::fs::File;
use parking_lot::RwLock;
use std::time::SystemTime;

pub struct Cache {
    inner: Box<dyn Backend + Send + Sync>,
    pool: Arc<CachePool>,
}

impl Cache {
    pub fn new(inner: Box<dyn Backend + Send + Sync>, pool: Arc<CachePool>) -> Self {
        Self {
            inner,
            pool,
        }
    }

    pub fn invalidate(&self, catalog: &str, track_id: u8) {
        self.pool.remove(&do_hash(format!("{}/{:02}", catalog, track_id)));
    }
}

#[async_trait]
impl Backend for Cache {
    async fn albums(&mut self) -> Result<HashSet<String>, BackendError> {
        // refresh should not be cached
        self.inner.albums().await
    }

    async fn get_audio(&self, album_id: &str, disc_id: u8, track_id: u8) -> Result<BackendReaderExt, BackendError> {
        self.pool.fetch(
            do_hash(format!("{}/{:02}/{:02}", album_id, disc_id, track_id)),
            self.inner.get_audio(album_id, disc_id, track_id),
        ).await
    }

    async fn get_cover(&self, catalog: &str) -> Result<BackendReader, BackendError> {
        // TODO: cache cover
        self.inner.get_cover(catalog).await
    }
}

pub struct CachePool {
    /// Root of cache folder
    root: PathBuf,
    /// Maximum space used by cache
    /// 0 means unlimited
    max_size: usize,
    cache: RwLock<HashMap<String, Arc<CacheItem>>>,
    last_used: RwLock<BTreeMap<String, u128>>,
}

impl CachePool {
    pub fn new<P: AsRef<Path>>(root: P, max_size: usize) -> Self {
        Self {
            root: PathBuf::from(root.as_ref()),
            max_size,
            cache: Default::default(),
            last_used: Default::default(),
        }
    }

    async fn fetch(
        &self,
        key: String,
        on_miss: impl Future<Output=Result<BackendReaderExt, BackendError>>,
    ) -> Result<BackendReaderExt, BackendError> {
        let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis();
        let item = if !self.has_cache(&key) {
            // calculate current space used
            let space_used = self.space_used();

            // prepare for new item
            let path = self.root.join(&key);
            let mut file = tokio::fs::File::create(&path).await?;
            let BackendReaderExt { extension, size, duration, mut reader } = on_miss.await?;
            let item = Arc::new(CacheItem::new(path, extension, size, duration, false));

            // write to map
            self.cache.write().insert(key.clone(), item.clone());
            self.last_used.write().insert(key.clone(), now);

            // remove old item if space is full
            if self.max_size != 0 && space_used > self.max_size {
                // get the first item of BTreeMap
                let read = self.last_used.read();
                let key = read.keys().next().unwrap();
                // remove it from last_used map
                // if key does not exist, it means other futures has removed it yet
                // so ignore here
                if let Some((key, ..)) = self.last_used.write().remove_entry(key) {
                    // remove it from cache map
                    // drop would do the removal
                    self.cache.write().remove(&key).unwrap().set_cached(false);
                }
            }

            // cache
            let item_spawn = item.clone();
            tokio::spawn(async move {
                let actual_size = tokio::io::copy(&mut reader, &mut file).await.unwrap() as usize;
                if item_spawn.size() != actual_size {
                    item_spawn.set_size(actual_size);
                }
                item_spawn.set_cached(true);
            });
            item
        } else {
            // update last_used time
            self.last_used.write().insert(key.clone(), now);
            self.cache.read().get(&key).unwrap().clone()
        };

        Ok(item.to_backend_reader_ext(tokio::fs::File::open(&item.path).await?))
    }

    fn remove(&self, key: &str) {
        self.cache.write().remove(key).map(|r| r.set_cached(false));
        self.last_used.write().remove(key);
    }

    fn has_cache(&self, key: &str) -> bool {
        self.last_used.read().contains_key(key)
    }

    fn space_used(&self) -> usize {
        self.cache.read().values().map(|a| a.size()).reduce(|a, b| a + b).unwrap_or(0)
    }
}

fn do_hash(key: String) -> String {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    Sha256::update(&mut hasher, key);
    let result = hasher.finalize();
    hex::encode(result)
}

struct CacheItem {
    ext: String,
    path: PathBuf,
    size: RwLock<usize>,
    duration: u64,
    cached: RwLock<bool>,
}

impl CacheItem {
    fn new(path: PathBuf, ext: String, size: usize, duration: u64, cached: bool) -> Self {
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

trait CacheReader {
    fn to_reader(&self, file: tokio::fs::File) -> CacheItemReader;

    fn to_backend_reader_ext(&self, file: tokio::fs::File) -> BackendReaderExt;
}

impl CacheReader for Arc<CacheItem> {
    fn to_reader(&self, file: tokio::fs::File) -> CacheItemReader {
        CacheItemReader {
            item: self.clone(),
            file: Box::pin(file),
            filled: 0,
            timer: None,
        }
    }

    fn to_backend_reader_ext(&self, file: File) -> BackendReaderExt {
        BackendReaderExt {
            extension: self.ext.clone(),
            size: self.size(),
            duration: self.duration,
            reader: Box::pin(self.to_reader(file)),
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
    file: Pin<Box<tokio::fs::File>>,
    filled: usize,

    timer: Option<Pin<Box<dyn Future<Output=()> + Send>>>,
}

impl AsyncRead for CacheItemReader {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<std::io::Result<()>> {
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
                            self.timer = Some(Box::pin(tokio::time::sleep(Duration::from_millis(100))));
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

#[cfg(test)]
mod test {
    use crate::cache::{Cache, CachePool};
    use crate::backends::drive::{DriveBackendSettings, DriveBackend};
    use std::path::PathBuf;
    use crate::Backend;
    use tokio::io::AsyncReadExt;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_cache() {
        panic!("cache test can not run properly!");

        let mut cache = Cache::new(
            Box::new(DriveBackend::new(Default::default(), DriveBackendSettings {
                corpora: "drive".to_string(),
                drive_id: Some("0AJIJiIDxF1yBUk9PVA".to_string()),
                token_path: "/tmp/anni_token".to_string(),
            }).await.unwrap()),
            Arc::new(CachePool {
                root: PathBuf::from("/tmp"),
                max_size: 0,
                cache: Default::default(),
                last_used: Default::default(),
            }),
        );
        cache.albums().await.unwrap();
        let mut reader = cache.get_audio("TGCS-10948", 1).await.unwrap();
        let mut r = Vec::new();
        reader.reader.read_to_end(&mut r).await.unwrap();
        let mut w = Vec::new();
        let mut file = tokio::fs::File::open("/tmp/90e369a90385e1c4467fe1d5dc3e3e69d8a0e24b05d0379b9131de6d579dbb08").await.unwrap();
        file.read_to_end(&mut w).await.unwrap();
        assert_eq!(r, w);
    }
}
