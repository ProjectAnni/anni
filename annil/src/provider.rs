use anni_provider::{AnniProvider, ProviderError};
use std::ops::{Deref, DerefMut};
use tokio::sync::RwLock;

pub struct AnnilProvider<T: AnniProvider + Send + Sync>(RwLock<T>);

impl<T: AnniProvider + Send + Sync> AnnilProvider<T> {
    pub fn new(provider: T) -> Self {
        Self(RwLock::new(provider))
    }

    pub async fn compute_etag(&self) -> Result<String, ProviderError> {
        let provider = self.0.read().await;

        let mut etag = 0;
        for album in provider.albums().await? {
            if let Ok(uuid) = uuid::Uuid::parse_str(album.as_ref()) {
                etag ^= uuid.as_u128();
            } else {
                log::error!("Failed to parse uuid: {album}");
            }
        }

        Ok(format!(r#""{}""#, base64::encode(etag.to_be_bytes())))
    }
}

impl<T: AnniProvider + Send + Sync> Deref for AnnilProvider<T> {
    type Target = RwLock<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: AnniProvider + Send + Sync> DerefMut for AnnilProvider<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
