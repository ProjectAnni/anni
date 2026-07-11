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

use std::time::Duration;

use thiserror::Error;

/// Runtime settings for the playback engine.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlayerConfig {
    pub output: OutputSettings,
    pub decode: DecodeSettings,
    pub preload: PreloadSettings,
}

impl PlayerConfig {
    pub fn validate(&self) -> Result<(), PlayerConfigError> {
        if self.output.buffer_duration.is_zero() {
            return Err(PlayerConfigError::ZeroBufferDuration);
        }
        if self.output.buffer_duration > OutputSettings::MAX_BUFFER_DURATION {
            return Err(PlayerConfigError::BufferDurationTooLarge);
        }
        if self.output.preferred_sample_rate == Some(0) {
            return Err(PlayerConfigError::ZeroSampleRate);
        }
        if self.output.preferred_channels == Some(0) {
            return Err(PlayerConfigError::ZeroChannels);
        }
        if self.preload.max_packets == 0 {
            return Err(PlayerConfigError::ZeroPreloadPackets);
        }
        if self.decode.recover_decode_errors && self.decode.max_consecutive_errors == 0 {
            return Err(PlayerConfigError::ZeroRecoverableDecodeErrors);
        }

        Ok(())
    }
}

/// Settings for the hardware output stream and its PCM buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputSettings {
    /// Preferred hardware sample rate. `None` uses the device default.
    pub preferred_sample_rate: Option<u32>,
    /// Preferred hardware channel count. `None` uses the device default.
    pub preferred_channels: Option<u16>,
    /// Amount of decoded PCM retained between the decoder and the audio callback.
    pub buffer_duration: Duration,
}

impl Default for OutputSettings {
    fn default() -> Self {
        Self {
            preferred_sample_rate: None,
            preferred_channels: Some(2),
            buffer_duration: Duration::from_millis(300),
        }
    }
}

impl OutputSettings {
    /// A defensive upper bound that prevents accidental multi-gigabyte rings.
    pub const MAX_BUFFER_DURATION: Duration = Duration::from_secs(30);
}

/// Settings passed to demuxers and decoders.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeSettings {
    /// Trim codec delay and padding when the decoder supports it.
    pub gapless: bool,
    /// Ask decoders to verify the stream when supported.
    pub verify: bool,
    /// Skip an isolated bad packet instead of stopping the whole track.
    pub recover_decode_errors: bool,
    /// Stop after this many consecutive packet decode errors.
    pub max_consecutive_errors: usize,
    pub seek_mode: SeekMode,
}

impl Default for DecodeSettings {
    fn default() -> Self {
        Self {
            gapless: true,
            verify: false,
            recover_decode_errors: true,
            max_consecutive_errors: 8,
            seek_mode: SeekMode::Accurate,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SeekMode {
    Coarse,
    #[default]
    Accurate,
}

/// Settings controlling how much of the next track is decoded in advance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreloadSettings {
    /// Minimum decoded duration to have ready before declaring a preload complete.
    pub target_duration: Duration,
    /// Safety bound for malformed streams or streams without useful timing information.
    pub max_packets: usize,
    /// At end-of-track, wait this long for an already-running preload to finish.
    pub gapless_wait_timeout: Duration,
}

impl Default for PreloadSettings {
    fn default() -> Self {
        Self {
            target_duration: Duration::from_millis(500),
            max_packets: 64,
            gapless_wait_timeout: Duration::from_millis(150),
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
pub enum PlayerConfigError {
    #[error("output buffer duration must be greater than zero")]
    ZeroBufferDuration,
    #[error("output buffer duration cannot exceed 30 seconds")]
    BufferDurationTooLarge,
    #[error("preferred sample rate must be greater than zero")]
    ZeroSampleRate,
    #[error("preferred channel count must be greater than zero")]
    ZeroChannels,
    #[error("preload max_packets must be greater than zero")]
    ZeroPreloadPackets,
    #[error("max_consecutive_errors must be greater than zero when recovery is enabled")]
    ZeroRecoverableDecodeErrors,
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{PlayerConfig, PlayerConfigError};

    #[test]
    fn rejects_zero_sized_runtime_settings() {
        let mut config = PlayerConfig::default();
        config.output.buffer_duration = Duration::ZERO;
        assert_eq!(
            config.validate(),
            Err(PlayerConfigError::ZeroBufferDuration)
        );

        config.output.buffer_duration = Duration::from_millis(1);
        config.preload.max_packets = 0;
        assert_eq!(
            config.validate(),
            Err(PlayerConfigError::ZeroPreloadPackets)
        );

        config.output.buffer_duration = Duration::from_secs(31);
        assert_eq!(
            config.validate(),
            Err(PlayerConfigError::BufferDurationTooLarge)
        );

        config.output.buffer_duration = Duration::from_millis(1);
        config.preload.max_packets = 1;
        config.decode.max_consecutive_errors = 0;
        assert_eq!(
            config.validate(),
            Err(PlayerConfigError::ZeroRecoverableDecodeErrors)
        );
    }
}
