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

pub mod config;
mod controls;
mod cpal_output;
mod decoder;
mod dsp;
pub mod stats;
mod utils;

pub use config::{
    DecodeSettings, OutputSettings, PlayerConfig, PlayerConfigError, PreloadSettings, SeekMode,
};
pub use controls::Controls;
pub use decoder::*;
pub use player::{
    AnniPlayer, AnniPlayerBuilder, AnniPlayerOptions, AnniPlayerStats, AudioCodec, AudioQuality,
    AudioVariant, Player, PlayerBuilder,
};
pub use sources::cached_http::OpenTrackError;
pub use stats::{CacheStats, PlaybackStatus, PlayerStats};
pub use types::{PlaybackError, PlaybackErrorKind, PlayerEvent, ProgressState};
pub mod player;
pub mod sources;
pub mod types;

pub use utils::create_unbound_channel;
