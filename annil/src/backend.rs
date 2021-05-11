use anni_backend::{BackendError, AnniBackend, BackendAudio, Backend};
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

    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn has_album(&self, catalog: &str) -> bool {
        self.albums.contains(catalog)
    }

    pub fn albums(&self) -> HashSet<&str> {
        self.albums.iter().map(|a| a.as_str()).collect()
    }

    pub async fn update_albums(&mut self) {
        // FIXME
        self.albums = self.inner.as_backend_mut().albums().await.unwrap();
    }

    pub fn set_enable(&mut self, enable: bool) {
        self.enabled = enable;
    }

    pub async fn get_audio(&self, catalog: &str, track_id: u8) -> Result<BackendAudio, BackendError> {
        self.inner.as_backend().get_audio(catalog, track_id).await
    }

    pub async fn get_cover(&self, catalog: &str) -> Result<impl AsyncRead, BackendError> {
        self.inner.as_backend().get_cover(catalog).await
    }
}
