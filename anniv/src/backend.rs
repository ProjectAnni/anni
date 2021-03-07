use anni_backend::{Backend, BackendError};
use std::pin::Pin;
use tokio::io::AsyncRead;

pub struct AnnivBackend {
    name: String,
    enabled: bool,
    inner: Box<dyn Backend + Send>,
}

impl AnnivBackend {
    pub async fn new(name: String, inner: Box<dyn Backend + Send>) -> Result<Self, BackendError> {
        let mut backend = Self {
            name,
            enabled: true,
            inner,
        };
        backend.inner.update_albums().await?;
        Ok(backend)
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn has_album(&self, catalog: &str) -> bool {
        self.inner.has(catalog)
    }

    pub fn albums(&self) -> Vec<&str> {
        self.inner.albums()
    }

    pub fn set_enable(&mut self, enable: bool) {
        self.enabled = enable;
    }

    pub async fn get_audio(&self, catalog: &str, track_id: u8, track_name: &str) -> Result<Pin<Box<dyn AsyncRead>>, BackendError> {
        self.inner.get_audio(catalog, track_id, track_name).await
    }
}