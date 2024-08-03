pub use anni_provider::providers::TypedPriorityProvider;

pub use crate::sources::cached_http::provider::AudioQuality;

use crossbeam::channel::Sender;
use reqwest::blocking::Client;

use std::{
    path::PathBuf,
    sync::{
        atomic::AtomicBool,
        mpsc::{self, Receiver},
        Arc, RwLock,
    },
    thread,
};

use anni_common::models::TrackIdentifier;

use crate::{
    sources::cached_http::{cache::CacheStore, provider::ProviderProxy, CachedAnnilSource},
    types::PlayerEvent,
    Controls, Decoder,
};

pub struct AnniPlayer {
    pub controls: Controls,
    pub client: Client,
    pub thread_killer: Sender<bool>,
    provider: RwLock<TypedPriorityProvider<ProviderProxy>>,
    cache_store: CacheStore, // root of cache
}

impl AnniPlayer {
    pub fn new(
        provider: TypedPriorityProvider<ProviderProxy>,
        cache_path: PathBuf,
    ) -> (Self, Receiver<PlayerEvent>) {
        let (controls, receiver, killer) = {
            let (sender, receiver) = mpsc::channel();
            let controls = Controls::new(sender);
            let thread_killer = crate::create_unbound_channel();

            thread::Builder::new()
                .name("anni-playback-decoder".to_owned())
                .spawn({
                    let controls = controls.clone();
                    move || {
                        let decoder = Decoder::new(controls, thread_killer.1);

                        decoder.start();
                    }
                })
                .unwrap();

            (controls, receiver, thread_killer.0)
        };

        (
            Self {
                controls,
                client: Client::new(),
                thread_killer: killer,
                provider: RwLock::new(provider),
                cache_store: CacheStore::new(cache_path),
            },
            receiver,
        )
    }

    pub fn add_provider(&self, url: String, auth: String, priority: i32) {
        let mut provider = self.provider.write().unwrap();

        provider.insert(ProviderProxy::new(url, auth, self.client.clone()), priority);
    }

    pub fn clear_provider(&self) {
        let mut provider = self.provider.write().unwrap();

        *provider = TypedPriorityProvider::new(vec![]);
    }

    pub fn open(&self, track: TrackIdentifier, quality: AudioQuality) -> anyhow::Result<()> {
        log::info!("loading track: {track}");

        self.controls.pause();

        let provider = self.provider.read().unwrap();

        let buffer_signal = Arc::new(AtomicBool::new(true));
        let source = CachedAnnilSource::new(
            track,
            quality,
            &self.cache_store,
            self.client.clone(),
            &provider,
            buffer_signal.clone(),
        )?;

        self.controls.open(Box::new(source), buffer_signal, false);

        Ok(())
    }

    pub fn open_and_play(
        &self,
        track: TrackIdentifier,
        quality: AudioQuality,
    ) -> anyhow::Result<()> {
        self.open(track, quality)?;
        self.play();

        Ok(())
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
        self.controls.open_file(path, false)
    }

    pub fn set_volume(&self, volume: f32) {
        self.controls.set_volume(volume);
    }

    pub fn seek(&self, position: u64) {
        self.controls.seek(position);
    }
}
