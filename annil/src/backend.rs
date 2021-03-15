use anni_backend::{BackendError, AnniBackend};
use std::pin::Pin;
use tokio::io::AsyncRead;
use std::borrow::Cow;

pub struct AnnilBackend {
    name: String,
    enabled: bool,
    inner: AnniBackend,
}

impl AnnilBackend {
    pub async fn new(name: String, inner: AnniBackend) -> Result<Self, BackendError> {
        let mut backend = Self {
            name,
            enabled: true,
            inner,
        };
        backend.inner.as_backend_mut().update_albums().await?;
        Ok(backend)
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub async fn has_album(&self, catalog: &str) -> bool {
        self.inner.as_backend().has(catalog).await
    }

    #[allow(clippy::needless_lifetimes)]
    pub async fn albums<'a>(&'a self) -> Vec<Cow<'a, str>> {
        self.inner.as_backend().albums().await
    }

    pub fn set_enable(&mut self, enable: bool) {
        self.enabled = enable;
    }

    pub async fn get_audio(&self, catalog: &str, track_id: u8) -> Result<Pin<Box<dyn AsyncRead>>, BackendError> {
        self.inner.as_backend().get_audio(catalog, track_id).await
    }

    pub async fn get_cover(&self, catalog: &str) -> Result<Pin<Box<dyn AsyncRead>>, BackendError> {
        self.inner.as_backend().get_cover(catalog).await
    }
}
