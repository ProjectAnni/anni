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

use std::sync::{atomic::AtomicBool, Arc};

pub use crossbeam::channel::{Receiver, Sender};
pub use symphonia_core::io::MediaSource;

/// Provides the current progress of the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgressState {
    /// The position, in milliseconds, of the player.
    pub position: u64,
    /// The duration, in milliseconds, of the file that
    /// is being played.
    pub duration: u64,
}

pub(crate) enum InternalPlayerEvent {
    Open(Box<dyn MediaSource>, Arc<AtomicBool>),
    Play,
    Pause,
    Stop,
    /// Called by `cpal_output` in the event the device outputting
    /// audio was changed/disconnected.
    DeviceChanged,
    Preload(Box<dyn MediaSource>, Arc<AtomicBool>),
    PlayPreloaded,
}

#[derive(Debug)]
pub enum PlayerEvent {
    /// Started playing
    Play,
    /// Paused
    Pause,
    /// Stopped
    Stop,
    /// Preload track is played. Should set next track to play
    PreloadPlayed,
    /// Playback progress updated
    Progress(ProgressState),
}
