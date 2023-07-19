use crate::{AnniProvider, AudioInfo, AudioResourceReader, ProviderError, Range, ResourceReader};
use async_trait::async_trait;
use std::borrow::Cow;
use std::collections::HashSet;
use std::num::NonZeroU8;

/// [MultipleProviders] combines multiple anni providers as a whole.
pub struct MultipleProviders(Vec<Box<dyn AnniProvider + Send + Sync>>);

impl MultipleProviders {
    pub fn new(providers: Vec<Box<dyn AnniProvider + Send + Sync>>) -> Self {
        Self(providers)
    }
}

#[async_trait]
impl AnniProvider for MultipleProviders {
    async fn albums(&self) -> crate::Result<HashSet<Cow<str>>> {
        let mut albums: HashSet<Cow<str>> = HashSet::new();
        for provider in self.0.iter() {
            albums.extend(provider.albums().await?);
        }
        Ok(albums)
    }

    async fn has_album(&self, album_id: &str) -> bool {
        for provider in self.0.iter() {
            if provider.has_album(album_id).await {
                return true;
            }
        }

        return false;
    }

    async fn get_audio_info(
        &self,
        album_id: &str,
        disc_id: NonZeroU8,
        track_id: NonZeroU8,
    ) -> crate::Result<AudioInfo> {
        for provider in self.0.iter() {
            if provider.has_album(album_id).await {
                return provider.get_audio_info(album_id, disc_id, track_id).await;
            }
        }

        Err(ProviderError::FileNotFound)
    }

    async fn get_audio(
        &self,
        album_id: &str,
        disc_id: NonZeroU8,
        track_id: NonZeroU8,
        range: Range,
    ) -> crate::Result<AudioResourceReader> {
        for provider in self.0.iter() {
            if provider.has_album(album_id).await {
                return provider.get_audio(album_id, disc_id, track_id, range).await;
            }
        }

        Err(ProviderError::FileNotFound)
    }

    async fn get_cover(
        &self,
        album_id: &str,
        disc_id: Option<NonZeroU8>,
    ) -> crate::Result<ResourceReader> {
        for provider in self.0.iter() {
            if provider.has_album(album_id).await {
                return provider.get_cover(album_id, disc_id).await;
            }
        }

        Err(ProviderError::FileNotFound)
    }

    async fn reload(&mut self) -> crate::Result<()> {
        let mut error = Ok(());
        for provider in self.0.iter_mut() {
            if let (Ok(()), Err(e)) = (&error, provider.reload().await) {
                error = Err(e);
            }
        }

        error
    }
}
