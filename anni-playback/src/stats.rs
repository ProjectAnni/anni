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

use std::sync::{
    atomic::{AtomicBool, AtomicU64, AtomicU8, AtomicUsize, Ordering},
    Arc, RwLock,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum PlaybackStatus {
    #[default]
    Idle = 0,
    Ready = 1,
    Playing = 2,
    Paused = 3,
    Stopped = 4,
    Error = 5,
}

impl PlaybackStatus {
    fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Ready,
            2 => Self::Playing,
            3 => Self::Paused,
            4 => Self::Stopped,
            5 => Self::Error,
            _ => Self::Idle,
        }
    }
}

/// A cheap, point-in-time snapshot of playback internals.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlayerStats {
    pub status: PlaybackStatus,
    pub decoded_packets: u64,
    pub decoded_frames: u64,
    pub preloaded_packets: u64,
    pub preloaded_frames: u64,
    pub recoverable_decode_errors: u64,
    pub output_samples: u64,
    pub dropped_samples: u64,
    pub underruns: u64,
    pub buffered_samples: usize,
    pub buffer_capacity_samples: usize,
    pub source_sample_rate: u32,
    pub source_channels: u16,
    pub output_sample_rate: u32,
    pub output_channels: u16,
    /// The active source is blocked waiting for network or disk data.
    pub source_is_buffering: bool,
    /// The hardware callback could not obtain enough decoded PCM.
    pub output_is_buffering: bool,
    /// True when either the source or output side is buffering.
    pub is_buffering: bool,
}

impl PlayerStats {
    pub fn buffered_duration_ms(&self) -> u64 {
        let samples_per_second =
            u64::from(self.output_sample_rate) * u64::from(self.output_channels);
        if samples_per_second == 0 {
            return 0;
        }

        (self.buffered_samples as u64 * 1000) / samples_per_second
    }
}

#[derive(Clone, Default)]
pub(crate) struct PlayerStatsHandle(Arc<PlayerStatsInner>);

#[derive(Default)]
struct PlayerStatsInner {
    status: AtomicU8,
    decoded_packets: AtomicU64,
    decoded_frames: AtomicU64,
    preloaded_packets: AtomicU64,
    preloaded_frames: AtomicU64,
    recoverable_decode_errors: AtomicU64,
    output_samples: AtomicU64,
    dropped_samples: AtomicU64,
    underruns: AtomicU64,
    buffered_samples: AtomicUsize,
    buffer_capacity_samples: AtomicUsize,
    source_sample_rate: AtomicU64,
    source_channels: AtomicU64,
    output_sample_rate: AtomicU64,
    output_channels: AtomicU64,
    output_is_buffering: AtomicBool,
    source_buffer_signal: RwLock<Option<Arc<AtomicBool>>>,
}

impl PlayerStatsHandle {
    pub(crate) fn snapshot(&self) -> PlayerStats {
        let inner = &self.0;
        let source_is_buffering = inner
            .source_buffer_signal
            .read()
            .unwrap()
            .as_ref()
            .is_some_and(|signal| signal.load(Ordering::Acquire));
        let output_is_buffering = inner.output_is_buffering.load(Ordering::Relaxed);
        PlayerStats {
            status: PlaybackStatus::from_u8(inner.status.load(Ordering::Relaxed)),
            decoded_packets: inner.decoded_packets.load(Ordering::Relaxed),
            decoded_frames: inner.decoded_frames.load(Ordering::Relaxed),
            preloaded_packets: inner.preloaded_packets.load(Ordering::Relaxed),
            preloaded_frames: inner.preloaded_frames.load(Ordering::Relaxed),
            recoverable_decode_errors: inner.recoverable_decode_errors.load(Ordering::Relaxed),
            output_samples: inner.output_samples.load(Ordering::Relaxed),
            dropped_samples: inner.dropped_samples.load(Ordering::Relaxed),
            underruns: inner.underruns.load(Ordering::Relaxed),
            buffered_samples: inner.buffered_samples.load(Ordering::Relaxed),
            buffer_capacity_samples: inner.buffer_capacity_samples.load(Ordering::Relaxed),
            source_sample_rate: inner.source_sample_rate.load(Ordering::Relaxed) as u32,
            source_channels: inner.source_channels.load(Ordering::Relaxed) as u16,
            output_sample_rate: inner.output_sample_rate.load(Ordering::Relaxed) as u32,
            output_channels: inner.output_channels.load(Ordering::Relaxed) as u16,
            source_is_buffering,
            output_is_buffering,
            is_buffering: source_is_buffering || output_is_buffering,
        }
    }

    pub(crate) fn set_status(&self, value: PlaybackStatus) {
        self.0.status.store(value as u8, Ordering::Relaxed);
    }

    pub(crate) fn decoded(&self, frames: usize) {
        self.0.decoded_packets.fetch_add(1, Ordering::Relaxed);
        self.0
            .decoded_frames
            .fetch_add(frames as u64, Ordering::Relaxed);
    }

    pub(crate) fn recoverable_decode_error(&self) {
        self.0
            .recoverable_decode_errors
            .fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn preloaded(&self, frames: usize) {
        self.0.preloaded_packets.fetch_add(1, Ordering::Relaxed);
        self.0
            .preloaded_frames
            .fetch_add(frames as u64, Ordering::Relaxed);
    }

    pub(crate) fn output_samples(&self, samples: usize) {
        self.0
            .output_samples
            .fetch_add(samples as u64, Ordering::Relaxed);
    }

    pub(crate) fn dropped_samples(&self, samples: usize) {
        self.0
            .dropped_samples
            .fetch_add(samples as u64, Ordering::Relaxed);
    }

    pub(crate) fn underrun(&self) {
        self.0.underruns.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn set_buffer(&self, len: usize, capacity: usize) {
        self.0.buffered_samples.store(len, Ordering::Relaxed);
        self.0
            .buffer_capacity_samples
            .store(capacity, Ordering::Relaxed);
    }

    pub(crate) fn set_source_format(&self, sample_rate: u32, channels: u16) {
        self.0
            .source_sample_rate
            .store(u64::from(sample_rate), Ordering::Relaxed);
        self.0
            .source_channels
            .store(u64::from(channels), Ordering::Relaxed);
    }

    pub(crate) fn clear_source_format(&self) {
        self.set_source_format(0, 0);
    }

    pub(crate) fn set_output_format(&self, sample_rate: u32, channels: u16) {
        self.0
            .output_sample_rate
            .store(u64::from(sample_rate), Ordering::Relaxed);
        self.0
            .output_channels
            .store(u64::from(channels), Ordering::Relaxed);
    }

    pub(crate) fn clear_output_format(&self) {
        self.set_output_format(0, 0);
    }

    pub(crate) fn set_output_buffering(&self, buffering: bool) {
        self.0
            .output_is_buffering
            .store(buffering, Ordering::Relaxed);
    }

    pub(crate) fn set_source_buffer_signal(&self, signal: Option<Arc<AtomicBool>>) {
        *self.0.source_buffer_signal.write().unwrap() = signal;
    }
}

/// A point-in-time snapshot of the on-disk HTTP cache.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub bytes_downloaded: u64,
    pub active_downloads: usize,
    pub completed_downloads: u64,
    pub failed_downloads: u64,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct CacheStatsHandle(Arc<CacheStatsInner>);

#[derive(Debug, Default)]
struct CacheStatsInner {
    hits: AtomicU64,
    misses: AtomicU64,
    bytes_downloaded: AtomicU64,
    active_downloads: AtomicUsize,
    completed_downloads: AtomicU64,
    failed_downloads: AtomicU64,
}

impl CacheStatsHandle {
    pub(crate) fn snapshot(&self) -> CacheStats {
        CacheStats {
            hits: self.0.hits.load(Ordering::Relaxed),
            misses: self.0.misses.load(Ordering::Relaxed),
            bytes_downloaded: self.0.bytes_downloaded.load(Ordering::Relaxed),
            active_downloads: self.0.active_downloads.load(Ordering::Relaxed),
            completed_downloads: self.0.completed_downloads.load(Ordering::Relaxed),
            failed_downloads: self.0.failed_downloads.load(Ordering::Relaxed),
        }
    }

    pub(crate) fn hit(&self) {
        self.0.hits.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn start_download(&self) {
        self.0.misses.fetch_add(1, Ordering::Relaxed);
        self.0.active_downloads.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn downloaded(&self, bytes: usize) {
        self.0
            .bytes_downloaded
            .fetch_add(bytes as u64, Ordering::Relaxed);
    }

    pub(crate) fn finish_download(&self, success: bool) {
        self.0.active_downloads.fetch_sub(1, Ordering::Relaxed);
        if success {
            self.0.completed_downloads.fetch_add(1, Ordering::Relaxed);
        } else {
            self.0.failed_downloads.fetch_add(1, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    use super::PlayerStatsHandle;

    #[test]
    fn distinguishes_source_and_output_buffering() {
        let stats = PlayerStatsHandle::default();
        let source = Arc::new(AtomicBool::new(true));
        stats.set_source_buffer_signal(Some(Arc::clone(&source)));

        let snapshot = stats.snapshot();
        assert!(snapshot.source_is_buffering);
        assert!(!snapshot.output_is_buffering);
        assert!(snapshot.is_buffering);

        source.store(false, Ordering::Relaxed);
        stats.set_output_buffering(true);
        let snapshot = stats.snapshot();
        assert!(!snapshot.source_is_buffering);
        assert!(snapshot.output_is_buffering);
        assert!(snapshot.is_buffering);
    }
}
