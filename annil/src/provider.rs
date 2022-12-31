use anni_provider::{AnniProvider, ProviderError, ResourceReader};
use std::borrow::Cow;
use std::collections::HashSet;
use std::num::NonZeroU8;
use std::ops::{Deref, DerefMut};
use uuid::Uuid;

pub struct AnnilProvider {
    name: String,
    inner: Box<dyn AnniProvider + Send + Sync>,
}

impl AnnilProvider {
    pub fn new(name: String, inner: Box<dyn AnniProvider + Send + Sync>) -> Self {
        Self { name, inner }
    }

    #[inline]
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub async fn has_album(&self, album_id: &Uuid) -> bool {
        let album_id = album_id.to_string();
        self.inner.has_album(&album_id).await
    }

    pub async fn get_cover(
        &self,
        album_id: &str,
        disc_id: Option<NonZeroU8>,
    ) -> Result<ResourceReader, ProviderError> {
        self.inner.get_cover(album_id, disc_id).await
    }

    pub async fn albums(&self) -> HashSet<Cow<'_, str>> {
        self.inner.albums().await.unwrap_or(HashSet::new())
    }
}

impl Deref for AnnilProvider {
    type Target = Box<dyn AnniProvider + Send + Sync>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for AnnilProvider {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
