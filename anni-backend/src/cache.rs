use crate::{Backend, BackendError, BackendReaderExt, BackendReader};
use std::path::PathBuf;
use std::collections::{HashSet, HashMap};
use async_trait::async_trait;
use std::sync::{Mutex, Arc};
use tokio::io::{AsyncRead, ReadBuf};
use std::task::{Context, Poll};
use std::pin::Pin;
use tokio::time::Duration;
use std::future::Future;
use tokio::fs::File;

#[macro_export]
macro_rules! cache {
    ($backend: expr, $s: expr) => {
        Cache::new($backend, $s)
    };
}

pub struct Cache {
    inner: Box<dyn Backend + Send + Sync>,
    pool: CachePool,
}

impl Cache {
    pub fn new(inner: impl Backend + Send + Sync + 'static, pool: CachePool) -> Self {
        Self {
            inner: Box::new(inner),
            pool,
        }
    }
}

#[async_trait]
impl Backend for Cache {
    async fn albums(&mut self) -> Result<HashSet<String>, BackendError> {
        // refresh should not be cached
        self.inner.albums().await
    }

    async fn get_audio(&self, catalog: &str, track_id: u8) -> Result<BackendReaderExt, BackendError> {
        self.pool.fetch(
            do_hash(format!("{}/{:02}", catalog, track_id)),
            self.inner.get_audio(catalog, track_id),
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
    /// Maximium space used by cache
    /// 0 means unlimited
    max_space: usize,
    cache: Arc<Mutex<HashMap<String, Arc<CacheItem>>>>,
}

impl CachePool {
    async fn fetch(&self, key: String, on_miss: impl Future<Output=Result<BackendReaderExt, BackendError>>)
                   -> Result<BackendReaderExt, BackendError> {
        let item = if !self.has_cache(&key) {
            // TODO: remove when cache space is full
            let path = self.root.join(&key);
            let mut file = tokio::fs::File::create(&path).await.unwrap();
            let BackendReaderExt { extension, size, mut reader } = on_miss.await?;
            let item = Arc::new(CacheItem::new(path, extension, size, false));
            self.cache.lock().unwrap().insert(key.clone(), item.clone());

            let item_spawn = item.clone();
            tokio::spawn(async move {
                tokio::io::copy(&mut reader, &mut file).await.unwrap();
                *item_spawn.cached.lock().unwrap() = true;
            });
            item
        } else {
            self.cache.lock().unwrap()
                .get(&key).unwrap().clone()
        };
        Ok(item.to_backend_reader_ext(tokio::fs::File::open(&item.path).await.unwrap()))
    }

    fn has_cache(&self, key: &str) -> bool {
        self.cache.lock().unwrap().contains_key(key)
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
    size: usize,
    cached: Mutex<bool>,
}

impl CacheItem {
    fn new(path: PathBuf, ext: String, size: usize, cached: bool) -> Self {
        CacheItem {
            path,
            ext,
            size,
            cached: Mutex::new(cached),
        }
    }

    fn cached(&self) -> bool {
        *self.cached.lock().unwrap()
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
            size: self.size,
            reader: Box::pin(self.to_reader(file)),
        }
    }
}

impl Drop for CacheItem {
    fn drop(&mut self) {
        // not cached, means:
        // a. file not fully cached and program reaches program termination
        // b. manually set cached to false
        if !*self.cached.lock().unwrap() {
            // TODO: handle error here?
            std::fs::remove_file(&self.path);
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
                            if self.filled != self.item.size {
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
                            // set up timer to wait 250ms
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

    #[tokio::test]
    async fn test_cache() {
        let mut cache = cache!(DriveBackend::new(Default::default(), DriveBackendSettings {
            corpora: "drive".to_string(),
            drive_id: Some("0AJIJiIDxF1yBUk9PVA".to_string()),
            token_path: "/tmp/anni_token".to_string(),
        }).await.unwrap(), CachePool{
            root: PathBuf::from("/tmp"),
            max_space: 0,
            cache: Default::default(),
        });
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
