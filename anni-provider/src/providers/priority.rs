use std::{borrow::Cow, collections::HashSet, num::NonZeroU8};

use async_trait::async_trait;

use crate::{AnniProvider, AudioResourceReader, ProviderError, Range, ResourceReader, Result};

pub type PriorityProvider = TypedPriorityProvider<Box<dyn AnniProvider + Send + Sync>>;

#[derive(Default)]
pub struct TypedPriorityProvider<P>(Vec<(i32, P)>);

impl<P> TypedPriorityProvider<P> {
    pub fn new(mut providers: Vec<(i32, P)>) -> Self {
        providers.sort_by(|(x, _), (y, _)| x.cmp(y).reverse());

        Self(providers)
    }

    pub fn insert(&mut self, provider: P, priority: i32) {
        match self.0.binary_search_by(|(p, _)| p.cmp(&priority).reverse()) {
            Ok(pos) | Err(pos) => self.0.insert(pos, (priority, provider)),
        };
    }

    pub fn iter(&self) -> impl Iterator<Item = &(i32, P)> + '_ {
        self.0.iter()
    }

    pub fn providers(&self) -> impl Iterator<Item = &P> + '_ {
        self.iter().map(|(_, provider)| provider)
    }
}

impl<P: AnniProvider + Send + Sync + 'static> TypedPriorityProvider<P> {
    pub fn into_boxed(self) -> PriorityProvider {
        self.0
            .into_iter()
            .map(|(priority, provider)| (priority, Box::new(provider) as _))
            .collect()
    }
}

impl<P> FromIterator<(i32, P)> for TypedPriorityProvider<P> {
    fn from_iter<T: IntoIterator<Item = (i32, P)>>(iter: T) -> Self {
        Self::new(iter.into_iter().collect())
    }
}

#[async_trait]
impl<P: AnniProvider + Send + Sync> AnniProvider for TypedPriorityProvider<P> {
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
            if let Ok(reader) = provider.get_audio(album_id, disc_id, track_id, range).await {
                return Ok(reader);
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
            if let Ok(reader) = provider.get_cover(album_id, disc_id).await {
                return Ok(reader);
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

#[cfg(test)]
mod test {
    use crate::{common::AnniProvider, providers::MultipleProviders};

    use super::PriorityProvider;

    fn generate_provider(priorities: Vec<i32>) -> PriorityProvider {
        priorities
            .into_iter()
            .map(|p| (p, Box::new(MultipleProviders::new(vec![])) as _))
            .collect()
    }

    fn get_priorities(provider: &PriorityProvider) -> Vec<i32> {
        provider.iter().map(|(p, _)| *p).collect::<Vec<_>>()
    }

    #[test]
    fn new() {
        let providers = generate_provider(vec![-5, 3, 2, 3]);
        assert_eq!(get_priorities(&providers), vec![3, 3, 2, -5]);
    }

    #[test]
    fn insert() {
        let mut providers = generate_provider(vec![1, 3, 9, -10, 1, 6, 0, 3, 3]);

        providers.insert(Box::new(MultipleProviders::new(vec![])), 8);
        assert_eq!(
            get_priorities(&providers),
            vec![9, 8, 6, 3, 3, 3, 1, 1, 0, -10]
        );

        providers.insert(Box::new(MultipleProviders::new(vec![])), 3);
        assert_eq!(
            get_priorities(&providers),
            vec![9, 8, 6, 3, 3, 3, 3, 1, 1, 0, -10]
        );

        providers.insert(Box::new(MultipleProviders::new(vec![])), 1);
        assert_eq!(
            get_priorities(&providers),
            vec![9, 8, 6, 3, 3, 3, 3, 1, 1, 1, 0, -10]
        );

        providers.insert(Box::new(MultipleProviders::new(vec![])), 10);
        assert_eq!(
            get_priorities(&providers),
            vec![10, 9, 8, 6, 3, 3, 3, 3, 1, 1, 1, 0, -10]
        );

        providers.insert(Box::new(MultipleProviders::new(vec![])), -912876510);
        assert_eq!(
            get_priorities(&providers),
            vec![10, 9, 8, 6, 3, 3, 3, 3, 1, 1, 1, 0, -10, -912876510]
        );
    }

    #[test]
    fn check_anni_provider_impl() {
        fn check<P: AnniProvider>() {}
        check::<PriorityProvider>();
    }
}
