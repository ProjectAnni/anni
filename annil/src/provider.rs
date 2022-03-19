use std::borrow::Cow;
use anni_provider::{ProviderError, AudioResourceReader, AnniProvider};
use tokio::io::AsyncRead;
use std::collections::HashSet;

pub struct AnnilProvider {
    name: String,
    enabled: bool,
    inner: Box<dyn AnniProvider + Send + Sync>,
}

impl AnnilProvider {
    pub async fn new(name: String, inner: Box<dyn AnniProvider + Send + Sync>, enable: bool) -> Result<Self, ProviderError> {
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
        self.albums().await.contains(album_id)
    }

    pub async fn albums(&self) -> HashSet<Cow<'_, str>> {
        if self.enabled {
            self.inner.albums().await.unwrap_or(HashSet::new())
        } else {
            HashSet::new()
        }
    }

    pub async fn reload(&mut self) -> Result<(), ProviderError> {
        log::debug!("[{}] Reloading provider albums", self.name());
        self.inner.reload().await?;
        Ok(())
    }

    pub async fn get_audio(&self, album_id: &str, disc_id: u8, track_id: u8, range: Option<String>) -> Result<AudioResourceReader, ProviderError> {
        log::trace!("[{}] Getting audio: {}/{}", self.name(), album_id, track_id);
        self.inner.get_audio(album_id, disc_id, track_id, range).await
    }

    pub async fn get_cover(&self, album_id: &str, disc_id: Option<u8>) -> Result<impl AsyncRead, ProviderError> {
        log::trace!("[{}] Getting cover: {}", self.name(), album_id);
        self.inner.get_cover(album_id, disc_id).await
    }
}