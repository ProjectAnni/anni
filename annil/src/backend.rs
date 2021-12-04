use anni_backend::{BackendError, AnniBackend, BackendReaderExt};
use tokio::io::AsyncRead;
use std::collections::{HashMap, HashSet};

pub struct AnnilBackend {
    name: String,
    enabled: bool,
    inner: AnniBackend,
    albums: HashMap<String, HashSet<String>>,
}

impl AnnilBackend {
    pub async fn new(name: String, mut inner: AnniBackend) -> Result<Self, BackendError> {
        let albums = inner.as_backend_mut().albums().await?;
        Ok(Self {
            name,
            enabled: true,
            inner,
            albums,
        })
    }

    #[inline]
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    #[inline]
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn has_album(&self, catalog: &str) -> bool {
        if self.albums.contains_key(catalog) {
            self.albums[catalog].len() == 1
        } else {
            self.albums().values().any(|albums| albums.contains(catalog))
        }
    }

    pub fn has_album_wide(&self, catalog: &str) -> bool {
        self.albums.contains_key(catalog) || self.albums().values().any(|albums| albums.contains(catalog))
    }

    pub fn albums(&self) -> HashMap<&str, HashSet<&str>> {
        self.albums
            .iter()
            .map(|(album, discs)|
                (album.as_str(), discs.iter().map(|d| d.as_str()).collect())
            ).collect()
    }

    pub async fn update_albums(&mut self) {
        // FIXME
        log::debug!("[{}] Updating backend albums", self.name());
        self.albums = self.inner.as_backend_mut().albums().await.unwrap();
    }

    #[inline]
    pub fn set_enable(&mut self, enable: bool) {
        self.enabled = enable;
    }

    pub async fn get_audio(&self, catalog: &str, track_id: u8) -> Result<BackendReaderExt, BackendError> {
        log::trace!("[{}] Getting audio: {}/{}", self.name(), catalog, track_id);
        self.inner.as_backend().get_audio(catalog, track_id).await
    }

    pub async fn get_cover(&self, catalog: &str) -> Result<impl AsyncRead, BackendError> {
        log::trace!("[{}] Getting cover: {}", self.name(), catalog);
        self.inner.as_backend().get_cover(catalog).await
    }
}
