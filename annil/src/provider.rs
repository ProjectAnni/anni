use std::borrow::Cow;
use anni_provider::{ProviderError, AnniProvider};
use std::collections::HashSet;
use std::ops::Deref;

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
}

impl Deref for AnnilProvider {
    type Target = Box<dyn AnniProvider + Send + Sync>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
