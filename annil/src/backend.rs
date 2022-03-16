use std::borrow::Cow;
use anni_provider::{ProviderError, AnniBackend, AudioResourceReader};
use tokio::io::AsyncRead;
use std::collections::HashSet;

pub struct AnnilBackend {
    name: String,
    enabled: bool,
    inner: AnniBackend,
}

impl AnnilBackend {
    pub async fn new(name: String, inner: AnniBackend, enable: bool) -> Result<Self, ProviderError> {
        Ok(Self {
            name,
            enabled: enable,
            inner,
        })
    }

    #[inline]
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub async fn has_album(&self, album_id: &str) -> bool {
        self.enabled && self.inner.contains_album(album_id).await
    }

    pub async fn albums(&self) -> HashSet<Cow<'_, str>> {
        if self.enabled {
            self.inner.as_backend().albums().await.unwrap_or(HashSet::new())
        } else {
            HashSet::new()
        }
    }

    pub async fn reload(&mut self) -> Result<(), ProviderError> {
        log::debug!("[{}] Reloading backend albums", self.name());
        self.inner.as_backend_mut().reload().await?;
        Ok(())
    }

    pub async fn get_audio(&self, album_id: &str, disc_id: u8, track_id: u8, range: Option<String>) -> Result<AudioResourceReader, ProviderError> {
        log::trace!("[{}] Getting audio: {}/{}", self.name(), album_id, track_id);
        self.inner.as_backend().get_audio(album_id, disc_id, track_id, range).await
    }

    pub async fn get_cover(&self, album_id: &str, disc_id: Option<u8>) -> Result<impl AsyncRead, ProviderError> {
        log::trace!("[{}] Getting cover: {}", self.name(), album_id);
        self.inner.as_backend().get_cover(album_id, disc_id).await
    }
}
