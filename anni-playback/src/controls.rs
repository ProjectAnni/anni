// This file is a part of simple_audio
// Copyright (c) 2022-2023 Erikas Taroza <erikastaroza@gmail.com>
//
// This program is free software: you can redistribute it and/or
// modify it under the terms of the GNU Lesser General Public License as
// published by the Free Software Foundation, either version 3 of
// the License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
// See the GNU Lesser General Public License for more details.
//
// You should have received a copy of the GNU Lesser General Public License along with this program.
// If not, see <https://www.gnu.org/licenses/>.

use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc, RwLock, RwLockReadGuard,
    },
};

use crossbeam::channel::unbounded;

use crate::{
    stats::{PlaybackStatus, PlayerStats, PlayerStatsHandle},
    types::*,
};

/// Creates a getter and setter for an AtomicBool.
macro_rules! getset_atomic_bool {
    ($name:ident, $setter_name:ident) => {
        pub fn $name(&self) -> bool {
            self.$name.load(std::sync::atomic::Ordering::Acquire)
        }

        pub fn $setter_name(&self, value: bool) {
            self.$name
                .store(value, std::sync::atomic::Ordering::Release);
        }
    };
}

/// Creates a getter and setter for an RwLock.
macro_rules! getset_rwlock {
    ($name:ident, $setter_name:ident, $lock_type:ty) => {
        pub fn $name(&self) -> RwLockReadGuard<'_, $lock_type> {
            self.$name.read().unwrap()
        }

        pub fn $setter_name(&self, value: $lock_type) {
            *self.$name.write().unwrap() = value;
        }
    };
}

type EventHandler = (Sender<InternalPlayerEvent>, Receiver<InternalPlayerEvent>);

#[derive(Clone)]
pub struct Controls {
    /// Decoder event channel
    event_handler: Arc<RwLock<EventHandler>>,
    is_playing: Arc<AtomicBool>,
    is_stopped: Arc<AtomicBool>,
    is_looping: Arc<AtomicBool>,
    is_normalizing: Arc<AtomicBool>,
    is_file_preloaded: Arc<AtomicBool>,
    output_enabled: Arc<AtomicBool>,
    volume: Arc<RwLock<f32>>,
    volume_bits: Arc<AtomicU32>,
    seek_ts: Arc<RwLock<Option<u64>>>,
    progress: Arc<RwLock<ProgressState>>,

    player_event_sender: Arc<std::sync::mpsc::Sender<PlayerEvent>>,
    stats: PlayerStatsHandle,
}

impl Controls {
    pub fn new(player_event_sender: std::sync::mpsc::Sender<PlayerEvent>) -> Self {
        Controls {
            event_handler: Arc::new(RwLock::new(unbounded())),
            is_playing: Arc::new(AtomicBool::new(false)),
            is_stopped: Arc::new(AtomicBool::new(true)),
            is_looping: Arc::new(AtomicBool::new(false)),
            is_normalizing: Arc::new(AtomicBool::new(false)),
            is_file_preloaded: Arc::new(AtomicBool::new(false)),
            output_enabled: Arc::new(AtomicBool::new(false)),
            volume: Arc::new(RwLock::new(1.0)),
            volume_bits: Arc::new(AtomicU32::new(1.0f32.to_bits())),
            seek_ts: Arc::new(RwLock::new(None)),
            progress: Arc::new(RwLock::new(ProgressState {
                position: 0,
                duration: 0,
            })),

            player_event_sender: Arc::new(player_event_sender),
            stats: PlayerStatsHandle::default(),
        }
    }

    pub fn open(
        &self,
        source: Box<dyn AnniSource>,
        buffer_signal: Arc<AtomicBool>,
        is_preload: bool,
    ) {
        if is_preload {
            self.send_internal_event(InternalPlayerEvent::Preload(source, buffer_signal));
        } else {
            self.send_internal_event(InternalPlayerEvent::Open(source, buffer_signal));
        }
    }

    pub fn open_file<P>(&self, path: P, is_preload: bool) -> anyhow::Result<()>
    where
        P: AsRef<Path>,
    {
        let buffer_signal = Arc::new(AtomicBool::new(false));
        let source = Box::new(std::fs::File::open(path)?);
        self.open(source, buffer_signal, is_preload);

        Ok(())
    }

    pub fn play_preloaded(&self) {
        self.send_internal_event(InternalPlayerEvent::PlayPreloaded);
    }

    pub(crate) fn event_handler(&self) -> RwLockReadGuard<'_, EventHandler> {
        self.event_handler.read().unwrap()
    }

    pub(crate) fn send_player_event(&self, event: PlayerEvent) {
        let _ = self.player_event_sender.send(event);
    }

    pub(crate) fn send_internal_event(&self, event: InternalPlayerEvent) {
        let _ = self.event_handler().0.send(event);
    }

    pub fn progress(&self) -> ProgressState {
        *self.progress.read().unwrap()
    }

    pub fn set_progress(&self, value: ProgressState) {
        let mut handle = self.progress.write().unwrap();
        if *handle != value {
            *handle = value;
            self.send_player_event(PlayerEvent::Progress(value));
        }
    }

    pub fn play(&self) {
        self.send_internal_event(InternalPlayerEvent::Play);
    }

    pub fn pause(&self) {
        self.send_internal_event(InternalPlayerEvent::Pause);
    }

    pub fn stop(&self) {
        self.send_internal_event(InternalPlayerEvent::Stop);
    }

    pub fn seek(&self, milliseconds: u64) {
        self.send_internal_event(InternalPlayerEvent::Seek(milliseconds));
    }

    pub fn shutdown(&self) {
        self.send_internal_event(InternalPlayerEvent::Shutdown);
    }

    pub fn stats(&self) -> PlayerStats {
        self.stats.snapshot()
    }

    pub fn set_volume(&self, value: f32) {
        let value = if value.is_finite() {
            value.clamp(0.0, 2.0)
        } else {
            1.0
        };
        *self.volume.write().unwrap() = value;
        self.volume_bits.store(value.to_bits(), Ordering::Relaxed);
    }

    /// Returns the current volume while preserving the original controls API.
    pub fn volume(&self) -> RwLockReadGuard<'_, f32> {
        self.volume.read().unwrap()
    }

    pub(crate) fn volume_value(&self) -> f32 {
        f32::from_bits(self.volume_bits.load(Ordering::Relaxed))
    }

    pub(crate) fn output_enabled(&self) -> bool {
        self.output_enabled.load(Ordering::Acquire)
    }

    pub(crate) fn set_output_enabled(&self, enabled: bool) {
        self.output_enabled.store(enabled, Ordering::Release);
    }

    pub(crate) fn stats_handle(&self) -> PlayerStatsHandle {
        self.stats.clone()
    }

    pub(crate) fn ready(&self, progress: ProgressState) {
        self.set_progress(progress);
        self.set_is_playing(false);
        self.set_is_stopped(false);
        self.stats.set_status(PlaybackStatus::Ready);
        self.send_player_event(PlayerEvent::Ready(progress));
    }

    pub(crate) fn playing(&self) {
        self.set_is_playing(true);
        self.set_is_stopped(false);
        self.stats.set_status(PlaybackStatus::Playing);
        self.send_player_event(PlayerEvent::Play);
    }

    pub(crate) fn paused(&self) {
        self.set_is_playing(false);
        self.set_is_stopped(false);
        self.stats.set_status(PlaybackStatus::Paused);
        self.send_player_event(PlayerEvent::Pause);
    }

    pub(crate) fn stopped(&self) {
        self.set_progress(ProgressState {
            position: 0,
            duration: 0,
        });
        self.set_is_playing(false);
        self.set_is_stopped(true);
        self.stats.set_status(PlaybackStatus::Stopped);
        self.send_player_event(PlayerEvent::Stop);
    }

    pub(crate) fn report_error(&self, error: PlaybackError) {
        if error.fatal {
            self.set_is_playing(false);
            self.set_is_stopped(true);
            self.set_output_enabled(false);
            self.stats.set_status(PlaybackStatus::Error);
        }
        self.send_player_event(PlayerEvent::Error(error));
    }

    pub(crate) fn set_buffering_realtime(&self, buffering: bool) {
        self.stats.set_output_buffering(buffering);
    }

    pub(crate) fn notify_buffering(&self, buffering: bool) {
        self.send_player_event(PlayerEvent::Buffering(buffering));
    }

    pub(crate) fn preload_ready(&self) {
        self.send_player_event(PlayerEvent::PreloadReady);
    }

    pub(crate) fn end_of_track(&self) {
        self.send_player_event(PlayerEvent::EndOfTrack);
    }

    pub(crate) fn preload_played(&self) {
        self.set_is_file_preloaded(false);
        self.send_player_event(PlayerEvent::PreloadPlayed);
    }

    getset_atomic_bool!(is_playing, set_is_playing);
    getset_atomic_bool!(is_stopped, set_is_stopped);
    getset_atomic_bool!(is_looping, set_is_looping);
    getset_atomic_bool!(is_normalizing, set_is_normalizing);
    getset_atomic_bool!(is_file_preloaded, set_is_file_preloaded);
    getset_rwlock!(seek_ts, set_seek_ts, Option<u64>);
}

#[cfg(test)]
mod tests {
    use super::Controls;

    #[test]
    fn commands_do_not_publish_optimistic_state_events() {
        let (sender, receiver) = std::sync::mpsc::channel();
        let controls = Controls::new(sender);
        controls.play();
        controls.seek(1_000);

        assert!(!controls.is_playing());
        assert_eq!(*controls.seek_ts(), None);
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn a_dropped_event_receiver_does_not_panic_the_player() {
        let (sender, receiver) = std::sync::mpsc::channel();
        let controls = Controls::new(sender);
        drop(receiver);

        controls.playing();
        controls.paused();
        controls.stopped();
    }

    #[test]
    fn volume_is_finite_and_bounded() {
        let (sender, _receiver) = std::sync::mpsc::channel();
        let controls = Controls::new(sender);
        controls.set_volume(f32::NAN);
        assert_eq!(controls.volume_value(), 1.0);
        controls.set_volume(10.0);
        assert_eq!(controls.volume_value(), 2.0);
    }
}
