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
use thiserror::Error;

pub use crossbeam::channel::{Receiver, Sender};
pub use symphonia_core::io::MediaSource;

pub use crate::sources::AnniSource;

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
    Open(Box<dyn AnniSource>, Arc<AtomicBool>),
    Play,
    Pause,
    Stop,
    /// Called by `cpal_output` in the event the device outputting
    /// audio was changed/disconnected.
    DeviceChanged,
    Preload(Box<dyn AnniSource>, Arc<AtomicBool>),
    PreloadFinished(u64),
    PlayPreloaded,
    Seek(u64),
    Shutdown,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
pub enum PlaybackErrorKind {
    #[error("output")]
    Output,
    #[error("source")]
    Source,
    #[error("decode")]
    Decode,
    #[error("seek")]
    Seek,
    #[error("preload")]
    Preload,
    #[error("internal")]
    Internal,
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
#[error("{kind} playback error: {message}")]
pub struct PlaybackError {
    pub kind: PlaybackErrorKind,
    pub message: String,
    pub fatal: bool,
}

impl PlaybackError {
    pub fn new(kind: PlaybackErrorKind, message: impl Into<String>, fatal: bool) -> Self {
        Self {
            kind,
            message: message.into(),
            fatal,
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerEvent {
    /// The current source was opened and is ready to play.
    Ready(ProgressState),
    /// Started playing
    Play,
    /// Paused
    Pause,
    /// Stopped
    Stop,
    /// Preload track is played. Should set next track to play
    PreloadPlayed,
    /// Enough of the next track has been decoded to switch safely.
    PreloadReady,
    /// The current source reached its natural end.
    EndOfTrack,
    /// Network or source buffering state changed.
    Buffering(bool),
    /// A recoverable or fatal playback error occurred.
    Error(PlaybackError),
    /// Playback progress updated
    Progress(ProgressState),
}
