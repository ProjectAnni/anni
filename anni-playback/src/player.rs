pub use crate::sources::cached_http::provider::{
    AudioCodec, AudioQuality, AudioVariant, ProviderProxy,
};
pub use anni_provider::providers::TypedPriorityProvider;

use crossbeam::channel::Sender;
use reqwest::blocking::Client;

use std::{
    ops::Deref,
    path::{Path, PathBuf},
    sync::{
        atomic::AtomicBool,
        mpsc::{self, Receiver},
        Arc, RwLock,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use anni_common::models::TrackIdentifier;

use crate::{
    config::{DecodeSettings, OutputSettings, PlayerConfig, PreloadSettings},
    sources::cached_http::{cache::CacheStore, CachedAnnilSource, OpenTrackError},
    stats::{CacheStats, PlayerStats},
    types::PlayerEvent,
    Controls, Decoder,
};

/// Low-level player owning the decoder thread and its lifecycle.
pub struct Player {
    controls: Controls,
    thread_killer: Sender<bool>,
    decoder_thread: Option<JoinHandle<()>>,
    decoder_done: Receiver<()>,
    config: PlayerConfig,
}

#[derive(Default)]
pub struct PlayerBuilder {
    config: PlayerConfig,
}

impl PlayerBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn config(mut self, config: PlayerConfig) -> Self {
        self.config = config;
        self
    }

    pub fn output_settings(mut self, settings: OutputSettings) -> Self {
        self.config.output = settings;
        self
    }

    pub fn decode_settings(mut self, settings: DecodeSettings) -> Self {
        self.config.decode = settings;
        self
    }

    pub fn preload_settings(mut self, settings: PreloadSettings) -> Self {
        self.config.preload = settings;
        self
    }

    pub fn buffer_duration(mut self, duration: Duration) -> Self {
        self.config.output.buffer_duration = duration;
        self
    }

    pub fn preferred_sample_rate(mut self, sample_rate: Option<u32>) -> Self {
        self.config.output.preferred_sample_rate = sample_rate;
        self
    }

    pub fn preferred_channels(mut self, channels: Option<u16>) -> Self {
        self.config.output.preferred_channels = channels;
        self
    }

    pub fn build(self) -> anyhow::Result<(Player, Receiver<PlayerEvent>)> {
        Player::build(self.config, false)
    }

    /// Defers opening the hardware device until the first `play()` command.
    pub fn build_lazy(self) -> anyhow::Result<(Player, Receiver<PlayerEvent>)> {
        Player::build(self.config, true)
    }
}

impl Player {
    pub fn builder() -> PlayerBuilder {
        PlayerBuilder::new()
    }

    pub fn try_new(config: PlayerConfig) -> anyhow::Result<(Self, Receiver<PlayerEvent>)> {
        Self::build(config, false)
    }

    fn build(
        config: PlayerConfig,
        lazy_output: bool,
    ) -> anyhow::Result<(Self, Receiver<PlayerEvent>)> {
        config.validate()?;
        let (event_sender, event_receiver) = mpsc::channel();
        let controls = Controls::new(event_sender);
        let (killer_sender, killer_receiver) = crate::create_unbound_channel();
        let decoder = if lazy_output {
            Decoder::with_config_lazy(controls.clone(), config.clone(), killer_receiver)
        } else {
            Decoder::try_with_config(controls.clone(), config.clone(), killer_receiver)?
        };
        let (done_sender, done_receiver) = mpsc::channel();
        let decoder_thread = thread::Builder::new()
            .name("anni-playback-decoder".to_owned())
            .spawn(move || {
                decoder.start();
                let _ = done_sender.send(());
            })?;

        Ok((
            Self {
                controls,
                thread_killer: killer_sender,
                decoder_thread: Some(decoder_thread),
                decoder_done: done_receiver,
                config,
            },
            event_receiver,
        ))
    }

    pub fn controls(&self) -> &Controls {
        &self.controls
    }

    pub fn config(&self) -> &PlayerConfig {
        &self.config
    }

    pub fn stats(&self) -> PlayerStats {
        self.controls.stats()
    }

    pub fn thread_killer(&self) -> Sender<bool> {
        self.thread_killer.clone()
    }

    pub fn shutdown(mut self) -> thread::Result<()> {
        self.request_shutdown();
        self.decoder_thread.take().map_or(Ok(()), JoinHandle::join)
    }

    fn request_shutdown(&self) {
        self.controls.shutdown();
        let _ = self.thread_killer.send(true);
    }
}

impl Deref for Player {
    type Target = Controls;

    fn deref(&self) -> &Self::Target {
        &self.controls
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.request_shutdown();
        if self
            .decoder_done
            .recv_timeout(Duration::from_millis(250))
            .is_ok()
            && let Some(thread) = self.decoder_thread.take()
        {
            let _ = thread.join();
        }
    }
}

pub struct AnniPlayer {
    /// Kept public for backwards compatibility. New code can use `Deref<Target = Controls>`.
    pub controls: Controls,
    pub client: Client,
    pub thread_killer: Sender<bool>,
    provider: RwLock<TypedPriorityProvider<ProviderProxy>>,
    cache_store: CacheStore,
    core: Player,
}

/// Legacy constructor options. Prefer `AnniPlayer::builder` for new code.
pub struct AnniPlayerOptions {
    pub sample_rate: u32,
    pub cache_path: PathBuf,
}

pub struct AnniPlayerBuilder {
    provider: TypedPriorityProvider<ProviderProxy>,
    cache_path: PathBuf,
    player_config: PlayerConfig,
    client: Option<Client>,
    network_timeout: Duration,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AnniPlayerStats {
    pub playback: PlayerStats,
    pub cache: CacheStats,
}

impl AnniPlayerBuilder {
    pub fn new(
        provider: TypedPriorityProvider<ProviderProxy>,
        cache_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            provider,
            cache_path: cache_path.into(),
            player_config: PlayerConfig::default(),
            client: None,
            network_timeout: Duration::from_secs(30),
        }
    }

    pub fn player_config(mut self, config: PlayerConfig) -> Self {
        self.player_config = config;
        self
    }

    pub fn output_settings(mut self, settings: OutputSettings) -> Self {
        self.player_config.output = settings;
        self
    }

    pub fn decode_settings(mut self, settings: DecodeSettings) -> Self {
        self.player_config.decode = settings;
        self
    }

    pub fn preload_settings(mut self, settings: PreloadSettings) -> Self {
        self.player_config.preload = settings;
        self
    }

    pub fn buffer_duration(mut self, duration: Duration) -> Self {
        self.player_config.output.buffer_duration = duration;
        self
    }

    pub fn preferred_sample_rate(mut self, sample_rate: Option<u32>) -> Self {
        self.player_config.output.preferred_sample_rate = sample_rate;
        self
    }

    pub fn preferred_channels(mut self, channels: Option<u16>) -> Self {
        self.player_config.output.preferred_channels = channels;
        self
    }

    pub fn network_timeout(mut self, timeout: Duration) -> Self {
        self.network_timeout = timeout;
        self
    }

    pub fn client(mut self, client: Client) -> Self {
        self.client = Some(client);
        self
    }

    pub fn build(self) -> anyhow::Result<(AnniPlayer, Receiver<PlayerEvent>)> {
        self.build_inner(false)
    }

    /// Defers opening the hardware device until the first `play()` command.
    pub fn build_lazy(self) -> anyhow::Result<(AnniPlayer, Receiver<PlayerEvent>)> {
        self.build_inner(true)
    }

    fn build_inner(self, lazy_output: bool) -> anyhow::Result<(AnniPlayer, Receiver<PlayerEvent>)> {
        let client = match self.client {
            Some(client) => client,
            None => Client::builder().timeout(self.network_timeout).build()?,
        };
        let (core, receiver) = Player::build(self.player_config, lazy_output)?;
        let controls = core.controls().clone();
        let thread_killer = core.thread_killer();

        Ok((
            AnniPlayer {
                controls,
                client,
                thread_killer,
                provider: RwLock::new(self.provider),
                cache_store: CacheStore::new(self.cache_path),
                core,
            },
            receiver,
        ))
    }
}

impl AnniPlayer {
    pub fn builder(
        provider: TypedPriorityProvider<ProviderProxy>,
        cache_path: impl Into<PathBuf>,
    ) -> AnniPlayerBuilder {
        AnniPlayerBuilder::new(provider, cache_path)
    }

    pub fn new(
        provider: TypedPriorityProvider<ProviderProxy>,
        options: AnniPlayerOptions,
    ) -> (Self, Receiver<PlayerEvent>) {
        let mut config = PlayerConfig::default();
        config.output.preferred_sample_rate = Some(options.sample_rate);
        Self::builder(provider, options.cache_path)
            .player_config(config)
            .build_lazy()
            .expect("legacy player construction failed")
    }

    pub fn try_new(
        provider: TypedPriorityProvider<ProviderProxy>,
        options: AnniPlayerOptions,
    ) -> anyhow::Result<(Self, Receiver<PlayerEvent>)> {
        let mut config = PlayerConfig::default();
        config.output.preferred_sample_rate = Some(options.sample_rate);
        Self::builder(provider, options.cache_path)
            .player_config(config)
            .build()
    }

    pub fn config(&self) -> &PlayerConfig {
        self.core.config()
    }

    pub fn stats(&self) -> AnniPlayerStats {
        AnniPlayerStats {
            playback: self.core.stats(),
            cache: self.cache_store.stats(),
        }
    }

    pub fn playback_stats(&self) -> PlayerStats {
        self.core.stats()
    }

    pub fn cache_stats(&self) -> CacheStats {
        self.cache_store.stats()
    }

    pub fn add_provider(&self, url: String, auth: String, priority: i32) {
        self.provider
            .write()
            .unwrap()
            .insert(ProviderProxy::new(url, auth, self.client.clone()), priority);
    }

    pub fn clear_provider(&self) {
        *self.provider.write().unwrap() = TypedPriorityProvider::new(vec![]);
    }

    pub fn open(
        &self,
        track: TrackIdentifier,
        quality: AudioQuality,
        opus: bool,
    ) -> Result<(), OpenTrackError> {
        self.open_variant(track, AudioVariant::from_legacy(quality, opus))
            .map(|_| ())
    }

    pub fn open_variant(
        &self,
        track: TrackIdentifier,
        variant: AudioVariant,
    ) -> Result<AudioVariant, OpenTrackError> {
        log::info!("loading track: {track} ({variant:?})");
        self.controls.pause();
        let provider = self.provider.read().unwrap();
        let buffer_signal = Arc::new(AtomicBool::new(true));
        let source = CachedAnnilSource::new_variant(
            track,
            variant,
            &self.cache_store,
            self.client.clone(),
            &provider,
            buffer_signal.clone(),
        )?;
        let effective_variant = source.variant();
        self.controls.open(Box::new(source), buffer_signal, false);
        Ok(effective_variant)
    }

    pub fn preload(
        &self,
        track: TrackIdentifier,
        variant: AudioVariant,
    ) -> Result<AudioVariant, OpenTrackError> {
        let provider = self.provider.read().unwrap();
        let buffer_signal = Arc::new(AtomicBool::new(true));
        let source = CachedAnnilSource::new_variant(
            track,
            variant,
            &self.cache_store,
            self.client.clone(),
            &provider,
            buffer_signal.clone(),
        )?;
        let effective_variant = source.variant();
        self.controls.open(Box::new(source), buffer_signal, true);
        Ok(effective_variant)
    }

    pub fn play_preloaded(&self) {
        self.controls.play_preloaded();
    }

    pub fn open_and_play(
        &self,
        track: TrackIdentifier,
        quality: AudioQuality,
        opus: bool,
    ) -> Result<(), OpenTrackError> {
        self.open(track, quality, opus)?;
        self.play();
        Ok(())
    }

    pub fn open_variant_and_play(
        &self,
        track: TrackIdentifier,
        variant: AudioVariant,
    ) -> Result<AudioVariant, OpenTrackError> {
        let effective = self.open_variant(track, variant)?;
        self.play();
        Ok(effective)
    }

    pub fn play(&self) {
        self.controls.play();
    }

    pub fn pause(&self) {
        self.controls.pause();
    }

    pub fn stop(&self) {
        self.controls.stop();
    }

    pub fn open_file(&self, path: String) -> anyhow::Result<()> {
        self.open_file_path(path)
    }

    pub fn open_file_path(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        self.controls.open_file(path, false)
    }

    pub fn preload_file(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        self.controls.open_file(path, true)
    }

    pub fn set_volume(&self, volume: f32) {
        self.controls.set_volume(volume);
    }

    pub fn seek(&self, position: u64) {
        self.controls.seek(position);
    }

    pub fn shutdown(self) -> thread::Result<()> {
        self.core.shutdown()
    }
}

impl Deref for AnniPlayer {
    type Target = Controls;

    fn deref(&self) -> &Self::Target {
        &self.controls
    }
}

#[cfg(test)]
mod tests {
    use super::Player;

    #[test]
    fn lazy_player_does_not_require_an_output_device() {
        let (player, _events) = Player::builder().build_lazy().unwrap();
        assert_eq!(player.stats().output_sample_rate, 0);
        player.shutdown().unwrap();
    }
}
