use std::{borrow::Cow, collections::HashSet, num::NonZeroU8};

use async_trait::async_trait;

use crate::{AnniProvider, AudioResourceReader, ProviderError, Range, ResourceReader, Result};

#[derive(Default)]
pub struct PriorityProvider(Vec<(i32, Box<dyn AnniProvider + Send + Sync>)>);

impl PriorityProvider {
    pub fn new(mut providers: Vec<(i32, Box<dyn AnniProvider + Send + Sync>)>) -> Self {
        providers.sort_by(|(x, _), (y, _)| x.cmp(y).reverse());

        Self(providers)
    }
}

impl FromIterator<(i32, Box<dyn AnniProvider + Send + Sync>)> for PriorityProvider {
    fn from_iter<T: IntoIterator<Item = (i32, Box<dyn AnniProvider + Send + Sync>)>>(iter: T) -> Self {
        Self::new(iter.into_iter().collect())
    }
}


#[async_trait]
impl AnniProvider for PriorityProvider {
    async fn albums(&self) -> Result<HashSet<Cow<str>>> {
        let mut res = HashSet::new();

        for (_, provider) in self.0.iter() {
            res.extend(provider.albums().await?);
        }

        Ok(res)
    }

    async fn get_audio(
        &self,
        album_id: &str,
        disc_id: NonZeroU8,
        track_id: NonZeroU8,
        range: Range,
    ) -> Result<AudioResourceReader> {
        for (_, provider) in self.0.iter() {
            match provider.get_audio(album_id, disc_id, track_id, range).await {
                Ok(reader) => return Ok(reader),
                _ => {}
            }
        }

        Err(ProviderError::FileNotFound)
    }

    async fn get_cover(
        &self,
        album_id: &str,
        disc_id: Option<NonZeroU8>,
    ) -> Result<ResourceReader> {
        for (_, provider) in self.0.iter() {
            match provider.get_cover(album_id, disc_id).await {
                Ok(reader) => return Ok(reader),
                _ => {}
            }
        }

        Err(ProviderError::FileNotFound)
    }

    /// Attempts to reload all providers.
    ///
    /// If multiple providers errors, the last error will be returned.
    async fn reload(&mut self) -> Result<()> {
        let mut error = None;

        for (_, provider) in self.0.iter_mut() {
            error.replace(provider.reload().await);
        }

        error.unwrap_or(Ok(()))
    }
}
