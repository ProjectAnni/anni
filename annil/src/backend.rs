use anni_backend::{BackendError, AnniBackend, BackendReaderExt};
use tokio::io::AsyncRead;
use std::collections::HashSet;

pub struct AnnilBackend {
    name: String,
    enabled: bool,
    inner: AnniBackend,
    albums: HashSet<String>,
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

    pub fn has_album(&self, album_id: &str) -> bool {
        self.albums.contains(album_id)
    }

    pub fn albums(&self) -> HashSet<&str> {
        self.albums.iter().map(|a| a.as_str()).collect()
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

    pub async fn get_audio(&self, album_id: &str, disc_id: u8, track_id: u8) -> Result<BackendReaderExt, BackendError> {
        log::trace!("[{}] Getting audio: {}/{}", self.name(), album_id, track_id);
        self.inner.as_backend().get_audio(album_id, disc_id, track_id).await
    }

    pub async fn get_cover(&self, album_id: &str) -> Result<impl AsyncRead, BackendError> {
        log::trace!("[{}] Getting cover: {}", self.name(), album_id);
        self.inner.as_backend().get_cover(album_id).await
    }
}
