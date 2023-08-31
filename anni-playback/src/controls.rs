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

use std::sync::{atomic::AtomicBool, Arc, RwLock, RwLockReadGuard};

use crossbeam::channel::{unbounded, Receiver, Sender};

use crate::types::*;

/// Creates a getter and setter for an AtomicBool.
macro_rules! getset_atomic_bool {
    ($name:ident, $setter_name:ident) => {
        pub fn $name(&self) -> bool {
            self.$name.load(std::sync::atomic::Ordering::SeqCst)
        }

        pub fn $setter_name(&self, value: bool) {
            self.$name.store(value, std::sync::atomic::Ordering::SeqCst);
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
    volume: Arc<RwLock<f32>>,
    seek_ts: Arc<RwLock<Option<u64>>>,
    progress: Arc<RwLock<ProgressState>>,

    player_event_sender: Arc<std::sync::mpsc::Sender<PlayerEvent>>,
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
            volume: Arc::new(RwLock::new(1.0)),
            seek_ts: Arc::new(RwLock::new(None)),
            progress: Arc::new(RwLock::new(ProgressState {
                position: 0,
                duration: 0,
            })),

            player_event_sender: Arc::new(player_event_sender),
        }
    }

    pub fn open(&self, source: Box<dyn MediaSource>, buffer_signal: Arc<AtomicBool>) {
        self.send_internal_event(InternalPlayerEvent::Open(source, buffer_signal));
    }

    pub fn preload(&self, source: Box<dyn MediaSource>, buffer_signal: Arc<AtomicBool>) {
        self.send_internal_event(InternalPlayerEvent::Preload(source, buffer_signal));
    }

    pub(crate) fn event_handler(&self) -> RwLockReadGuard<'_, EventHandler> {
        self.event_handler.read().unwrap()
    }

    fn send_player_event(&self, event: PlayerEvent) {
        self.player_event_sender.send(event).unwrap();
    }

    fn send_internal_event(&self, event: InternalPlayerEvent) {
        self.event_handler().0.send(event).unwrap();
    }

    pub fn progress(&self) -> ProgressState {
        self.progress.read().unwrap().clone()
    }

    pub fn set_progress(&self, value: ProgressState) {
        let mut handle = self.progress.write().unwrap();
        if *handle != value {
            *handle = value;
            self.send_player_event(PlayerEvent::Progress(value));
        }
    }

    pub fn play(&self) {
        if self.is_playing() {
            return;
        }

        self.send_internal_event(InternalPlayerEvent::Play);
        self.send_player_event(PlayerEvent::Play);
        self.set_is_playing(true);
        self.set_is_stopped(false);
    }

    pub fn play_preload(&self) {
        self.send_internal_event(InternalPlayerEvent::PlayPreload);
    }

    pub fn pause(&self) {
        if !self.is_playing() {
            return;
        }

        self.send_internal_event(InternalPlayerEvent::Pause);
        self.send_player_event(PlayerEvent::Pause);
        self.set_is_playing(false);
        self.set_is_stopped(false);
    }

    pub fn stop(&self) {
        if self.is_stopped() {
            return;
        }

        self.send_internal_event(InternalPlayerEvent::Stop);

        let progress = ProgressState {
            position: 0,
            duration: 0,
        };

        self.set_progress(progress);
        self.send_player_event(PlayerEvent::Pause);
        self.set_is_playing(false);
        self.set_is_stopped(true);
    }

    pub fn seek(&self, milliseconds: u64) {
        self.set_seek_ts(Some(milliseconds));
        self.set_progress(ProgressState {
            position: milliseconds,
            duration: self.progress().duration,
        });
    }

    pub fn done(&self) {
        self.send_player_event(PlayerEvent::Done);
    }

    getset_atomic_bool!(is_playing, set_is_playing);
    getset_atomic_bool!(is_stopped, set_is_stopped);
    getset_atomic_bool!(is_looping, set_is_looping);
    getset_atomic_bool!(is_normalizing, set_is_normalizing);
    getset_atomic_bool!(is_file_preloaded, set_is_file_preloaded);
    getset_rwlock!(volume, set_volume, f32);
    getset_rwlock!(seek_ts, set_seek_ts, Option<u64>);
}
