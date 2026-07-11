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
    collections::VecDeque,
    sync::{atomic::AtomicBool, Arc},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use anyhow::{anyhow, Context};
use once_cell::sync::Lazy;
use symphonia::{
    core::{
        audio::{AsGenericAudioBufferRef, Audio, AudioBuffer},
        codecs::{
            audio::{AudioDecoder, AudioDecoderOptions},
            registry::CodecRegistry,
        },
        formats::{
            probe::Hint, FormatOptions, FormatReader, SeekMode as SymphoniaSeekMode, SeekTo,
            TrackType,
        },
        io::MediaSourceStream,
        meta::MetadataOptions,
        units::{Time, TimeBase, Timestamp},
    },
    default::{self, register_enabled_codecs},
};

use super::opus::OpusDecoder;
use crate::{
    config::{PlayerConfig, SeekMode},
    controls::Controls,
    cpal_output::{CpalOutput, CpalOutputStream},
    sources::AnniSource,
    types::{InternalPlayerEvent, PlaybackError, PlaybackErrorKind, ProgressState, Receiver},
};

enum PlaybackState {
    Playing,
    Completed,
    Idle,
}

pub static CODEC_REGISTRY: Lazy<CodecRegistry> = Lazy::new(|| {
    let mut registry = CodecRegistry::new();
    register_enabled_codecs(&mut registry);
    registry.register_audio_decoder::<OpusDecoder>();
    registry
});

pub struct Decoder {
    thread_killer: Receiver<bool>,
    event_receiver: Receiver<InternalPlayerEvent>,
    controls: Controls,
    state: DecoderState,
    cpal_output_stream: Option<CpalOutputStream>,
    cpal_output: Option<CpalOutput>,
    playback: Option<Playback>,
    preload_playback: Option<Playback>,
    preload_thread: Option<JoinHandle<anyhow::Result<Playback>>>,
    preload_cancel: Option<Arc<AtomicBool>>,
    preload_generation: u64,
    config: PlayerConfig,
    startup_error: Option<String>,
    reported_buffering: bool,
}

impl Decoder {
    /// Backwards-compatible constructor. Output initialization errors are
    /// reported through `PlayerEvent::Error` when `start` runs.
    pub fn new(controls: Controls, sample_rate: u32, thread_killer: Receiver<bool>) -> Self {
        let mut config = PlayerConfig::default();
        config.output.preferred_sample_rate = Some(sample_rate);
        Self::with_config_lazy(controls, config, thread_killer)
    }

    pub fn try_with_config(
        controls: Controls,
        config: PlayerConfig,
        thread_killer: Receiver<bool>,
    ) -> anyhow::Result<Self> {
        config.validate()?;
        let event_receiver = controls.event_handler().1.clone();
        let output = CpalOutputStream::new(&config.output, controls.clone())?;

        Ok(Self {
            thread_killer,
            event_receiver,
            controls,
            state: DecoderState::Idle,
            cpal_output_stream: Some(output),
            cpal_output: None,
            playback: None,
            preload_playback: None,
            preload_thread: None,
            preload_cancel: None,
            preload_generation: 0,
            config,
            startup_error: None,
            reported_buffering: false,
        })
    }

    pub fn with_config_lazy(
        controls: Controls,
        config: PlayerConfig,
        thread_killer: Receiver<bool>,
    ) -> Self {
        let event_receiver = controls.event_handler().1.clone();
        let startup_error = config.validate().err().map(|error| error.to_string());

        Self {
            thread_killer,
            event_receiver,
            controls,
            state: DecoderState::Idle,
            cpal_output_stream: None,
            cpal_output: None,
            playback: None,
            preload_playback: None,
            preload_thread: None,
            preload_cancel: None,
            preload_generation: 0,
            config,
            startup_error,
            reported_buffering: false,
        }
    }

    pub fn start(mut self) {
        if let Some(error) = self.startup_error.take() {
            self.controls.report_error(PlaybackError::new(
                PlaybackErrorKind::Internal,
                error,
                true,
            ));
        }

        loop {
            if let Err(error) = self.poll_preload_thread() {
                self.controls.report_error(PlaybackError::new(
                    PlaybackErrorKind::Preload,
                    error.to_string(),
                    false,
                ));
            }

            match self.listen_for_message() {
                Ok(true) => break,
                Ok(false) => {}
                Err(error) => {
                    self.controls.report_error(PlaybackError::new(
                        PlaybackErrorKind::Internal,
                        error.to_string(),
                        false,
                    ));
                }
            }

            match self.do_playback() {
                Ok(PlaybackState::Completed) => {
                    self.controls.end_of_track();
                    if let Err(error) = self.finish_playback(false) {
                        self.controls.report_error(PlaybackError::new(
                            PlaybackErrorKind::Output,
                            error.to_string(),
                            true,
                        ));
                        self.stop_internal(true);
                    }
                }
                Ok(_) => {}
                Err(error) => {
                    self.controls.report_error(PlaybackError::new(
                        PlaybackErrorKind::Decode,
                        error.to_string(),
                        true,
                    ));
                    self.stop_internal(true);
                }
            }
            self.publish_buffering_state();
        }

        self.stop_internal(false);
    }

    fn listen_for_message(&mut self) -> anyhow::Result<bool> {
        let message = if self.state.is_idle() || self.state.is_paused() {
            crossbeam::select! {
                recv(self.thread_killer) -> killer => {
                    if killer.is_ok() {
                        return Ok(true);
                    }
                    // Preserve the legacy Decoder API: dropping an unused
                    // killer sender must not terminate playback.
                    self.event_receiver.recv().ok()
                },
                recv(self.event_receiver) -> message => message.ok(),
            }
        } else {
            if self.thread_killer.try_recv().is_ok() {
                return Ok(true);
            }
            self.event_receiver.try_recv().ok()
        };

        let Some(message) = message else {
            return Ok(false);
        };

        match message {
            InternalPlayerEvent::Open(source, buffer_signal) => {
                self.clear_preload();
                self.pause_output();
                self.clear_output_buffer();
                self.cpal_output = None;
                self.controls.set_seek_ts(None);
                let stats = self.controls.stats_handle();
                stats.clear_source_format();
                stats.set_source_buffer_signal(Some(Arc::clone(&buffer_signal)));

                match Self::open(source, buffer_signal, &self.config) {
                    Ok(playback) => {
                        let progress = ProgressState {
                            position: 0,
                            duration: playback.duration,
                        };
                        self.playback = Some(playback);
                        self.state = DecoderState::Paused;
                        self.controls.ready(progress);
                    }
                    Err(error) => {
                        self.playback = None;
                        self.state = DecoderState::Idle;
                        self.controls.stats_handle().set_source_buffer_signal(None);
                        self.controls.set_progress(ProgressState {
                            position: 0,
                            duration: 0,
                        });
                        self.controls.report_error(PlaybackError::new(
                            PlaybackErrorKind::Source,
                            error.to_string(),
                            true,
                        ));
                    }
                }
            }
            InternalPlayerEvent::Play => {
                if self.state.is_playing() {
                    // Repeated play commands are idempotent.
                } else if self.playback.is_none() {
                    self.controls.report_error(PlaybackError::new(
                        PlaybackErrorKind::Source,
                        "cannot play without an open source",
                        false,
                    ));
                } else {
                    self.controls.set_output_enabled(true);
                    let result = self.ensure_output_stream().and_then(|_| {
                        self.cpal_output_stream
                            .as_ref()
                            .context("output stream is unavailable")?
                            .play()
                    });
                    match result {
                        Ok(()) => {
                            self.state = DecoderState::Playing;
                            self.controls.playing();
                        }
                        Err(error) => self.controls.report_error(PlaybackError::new(
                            PlaybackErrorKind::Output,
                            error.to_string(),
                            false,
                        )),
                    }
                    if !self.state.is_playing() {
                        self.controls.set_output_enabled(false);
                    }
                }
            }
            InternalPlayerEvent::Pause => {
                if self.playback.is_some() && !self.state.is_paused() {
                    self.pause_output();
                    self.state = DecoderState::Paused;
                    self.controls.paused();
                }
            }
            InternalPlayerEvent::Stop => {
                if !self.state.is_idle() || self.playback.is_some() {
                    self.stop_internal(true);
                }
            }
            InternalPlayerEvent::DeviceChanged => {
                self.controls.set_output_enabled(false);
                self.cpal_output = None;
                self.cpal_output_stream = None;
                let stats = self.controls.stats_handle();
                stats.set_buffer(0, 0);
                stats.clear_output_format();
                self.state = DecoderState::Paused;
                self.controls.paused();
            }
            InternalPlayerEvent::OutputFailed => {
                self.stop_internal(true);
                // The callback permanently cancels the old ring writer to
                // unblock a decoder waiting on a full buffer. Drop that stream
                // so the next play command gets a fresh writer and device.
                self.cpal_output_stream = None;
            }
            InternalPlayerEvent::Preload(source, buffer_signal) => {
                self.preload_playback = None;
                self.controls.set_is_file_preloaded(false);
                if let Some(cancel) = self.preload_cancel.take() {
                    cancel.store(true, std::sync::atomic::Ordering::Release);
                }
                self.preload_generation = self.preload_generation.wrapping_add(1);
                let generation = self.preload_generation;
                let cancel = Arc::new(AtomicBool::new(false));
                self.preload_thread = Some(Self::preload(
                    source,
                    buffer_signal,
                    self.config.clone(),
                    Arc::clone(&cancel),
                    self.controls.stats_handle(),
                    self.controls.clone(),
                    generation,
                ));
                self.preload_cancel = Some(cancel);
            }
            InternalPlayerEvent::PreloadFinished(generation) => {
                if generation == self.preload_generation
                    && let Err(error) = self.complete_preload_thread()
                {
                    self.controls.report_error(PlaybackError::new(
                        PlaybackErrorKind::Preload,
                        error.to_string(),
                        false,
                    ));
                }
            }
            InternalPlayerEvent::PlayPreloaded => {
                if let Err(error) = self.poll_preload_thread() {
                    self.controls.report_error(PlaybackError::new(
                        PlaybackErrorKind::Preload,
                        error.to_string(),
                        false,
                    ));
                    return Ok(false);
                }
                if self.preload_playback.is_some() {
                    if let Err(error) = self.finish_playback(true) {
                        self.controls.set_output_enabled(false);
                        self.state = DecoderState::Paused;
                        self.controls.paused();
                        self.controls.report_error(PlaybackError::new(
                            PlaybackErrorKind::Output,
                            error.to_string(),
                            false,
                        ));
                    }
                } else {
                    self.controls.report_error(PlaybackError::new(
                        PlaybackErrorKind::Preload,
                        "next track is not ready",
                        false,
                    ));
                }
            }
            InternalPlayerEvent::Seek(position) => {
                // The timestamp is stored by Controls. Handling it in do_playback
                // keeps reader and decoder mutation on the decoder thread.
                self.controls.set_seek_ts(Some(position));
            }
            InternalPlayerEvent::Shutdown => return Ok(true),
        }

        Ok(false)
    }

    fn do_playback(&mut self) -> anyhow::Result<PlaybackState> {
        if self.playback.is_none() {
            return Ok(PlaybackState::Idle);
        }

        let seek_applied = match self.apply_pending_seek() {
            Ok(applied) => applied,
            Err(error) => {
                self.controls.report_error(PlaybackError::new(
                    PlaybackErrorKind::Seek,
                    error.to_string(),
                    false,
                ));
                false
            }
        };
        if seek_applied {
            return Ok(if self.state.is_playing() {
                PlaybackState::Playing
            } else {
                PlaybackState::Idle
            });
        }

        if !self.state.is_playing() {
            return Ok(PlaybackState::Idle);
        }

        self.ensure_output_stream()?;

        if let Some(preloaded) = self
            .playback
            .as_mut()
            .and_then(|playback| playback.preload.pop_front())
        {
            self.controls.set_progress(ProgressState {
                position: preloaded.position,
                duration: self.playback.as_ref().unwrap().duration,
            });
            self.write_audio_buffer(
                preloaded.buffer.as_generic_audio_buffer_ref(),
                preloaded.buffer.capacity() as u64,
            )?;
            return Ok(PlaybackState::Playing);
        }

        let packet = loop {
            let playback = self.playback.as_mut().unwrap();
            match playback.reader.next_packet() {
                Ok(Some(packet)) if packet.track_id == playback.track_id => break packet,
                Ok(Some(_)) => continue,
                Ok(None) => {
                    if self.controls.is_looping() {
                        self.controls.set_seek_ts(Some(0));
                        return Ok(PlaybackState::Playing);
                    }
                    return Ok(PlaybackState::Completed);
                }
                Err(error) => return Err(error.into()),
            }
        };

        let playback = self.playback.as_mut().unwrap();
        let playback_duration = playback.duration;
        let timebase = playback.timebase;
        let packet_position = timestamp_to_millis(timebase, packet.pts);
        let decoded = match playback.decoder.decode(&packet) {
            Ok(decoded) => {
                playback.consecutive_decode_errors = 0;
                decoded
            }
            Err(error) if self.config.decode.recover_decode_errors => {
                playback.consecutive_decode_errors += 1;
                if playback.consecutive_decode_errors > self.config.decode.max_consecutive_errors {
                    return Err(error).context("too many consecutive audio decode errors");
                }
                self.controls.stats_handle().recoverable_decode_error();
                self.controls.report_error(PlaybackError::new(
                    PlaybackErrorKind::Decode,
                    error.to_string(),
                    false,
                ));
                return Ok(PlaybackState::Playing);
            }
            Err(error) => return Err(error).context("Could not decode audio packet"),
        };

        let decoded_frames = decoded.frames();
        self.controls.stats_handle().decoded(decoded_frames);
        let seek_target = playback.seek_target;
        let discard_frames = seek_target.map_or(0, |target| {
            frames_between_timestamps(timebase, packet.pts, target, decoded.spec().rate())
        });
        if seek_target.is_some() && discard_frames >= decoded_frames {
            // Accurate seeking requires decoding from the preceding packet so
            // codec state is reconstructed, but those frames must not be heard.
            return Ok(PlaybackState::Playing);
        }

        let position = seek_target
            .map(|target| timestamp_to_millis(timebase, target))
            .unwrap_or(packet_position);
        playback.seek_target = None;

        if discard_frames > 0 {
            let spec = decoded.spec().clone();
            let mut trimmed = AudioBuffer::<f32>::new(spec, decoded.capacity());
            trimmed.resize_uninit(decoded_frames);
            decoded.copy_to(&mut trimmed);
            trimmed.trim(discard_frames, 0);

            self.controls.set_progress(ProgressState {
                position,
                duration: playback_duration,
            });
            if self
                .cpal_output
                .as_ref()
                .is_some_and(|output| !output.matches_spec(trimmed.spec()))
            {
                if let Some(output) = self.cpal_output.as_mut() {
                    output.flush();
                }
                self.cpal_output = None;
            }
            let duration = trimmed.capacity() as u64;
            if self.cpal_output.is_none() {
                let output = self
                    .cpal_output_stream
                    .as_ref()
                    .context("output stream is unavailable")?
                    .create_output(trimmed.spec().clone(), duration)?;
                self.cpal_output = Some(output);
            }
            self.cpal_output
                .as_mut()
                .unwrap()
                .write(trimmed.as_generic_audio_buffer_ref());
            return Ok(PlaybackState::Playing);
        }

        self.controls.set_progress(ProgressState {
            position,
            duration: playback_duration,
        });

        if self
            .cpal_output
            .as_ref()
            .is_some_and(|output| !output.matches_spec(decoded.spec()))
        {
            if let Some(output) = self.cpal_output.as_mut() {
                output.flush();
            }
            self.cpal_output = None;
        }
        let duration = decoded.capacity() as u64;
        if self.cpal_output.is_none() {
            let output = self
                .cpal_output_stream
                .as_ref()
                .context("output stream is unavailable")?
                .create_output(decoded.spec().clone(), duration)?;
            self.cpal_output = Some(output);
        }
        self.cpal_output.as_mut().unwrap().write(decoded);
        Ok(PlaybackState::Playing)
    }

    fn apply_pending_seek(&mut self) -> anyhow::Result<bool> {
        let seek_ms = *self.controls.seek_ts();
        let Some(seek_ms) = seek_ms else {
            return Ok(false);
        };
        self.controls.set_seek_ts(None);

        let (timebase, duration, actual_ts) = {
            let playback = self.playback.as_mut().unwrap();
            let seek_mode = match self.config.decode.seek_mode {
                SeekMode::Coarse => SymphoniaSeekMode::Coarse,
                SeekMode::Accurate => SymphoniaSeekMode::Accurate,
            };
            let result = playback.reader.seek(
                seek_mode,
                SeekTo::Time {
                    time: Time::from_millis_u64(seek_ms),
                    track_id: Some(playback.track_id),
                },
            )?;
            playback.decoder.reset();
            playback.preload.clear();
            playback.consecutive_decode_errors = 0;
            playback.seek_target = matches!(self.config.decode.seek_mode, SeekMode::Accurate)
                .then_some(result.required_ts)
                .filter(|required| *required > result.actual_ts);
            let visible_ts = if matches!(self.config.decode.seek_mode, SeekMode::Accurate) {
                result.required_ts
            } else {
                result.actual_ts
            };
            (playback.timebase, playback.duration, visible_ts)
        };
        self.cpal_output = None;
        self.clear_output_buffer();

        let actual = timestamp_to_millis(timebase, actual_ts);
        self.controls.set_progress(ProgressState {
            position: actual,
            duration,
        });
        Ok(true)
    }

    fn write_audio_buffer(
        &mut self,
        decoded: symphonia::core::audio::GenericAudioBufferRef<'_>,
        duration: u64,
    ) -> anyhow::Result<()> {
        self.write_decoded(decoded, duration)
    }

    fn write_decoded(
        &mut self,
        decoded: symphonia::core::audio::GenericAudioBufferRef<'_>,
        duration: u64,
    ) -> anyhow::Result<()> {
        if self
            .cpal_output
            .as_ref()
            .is_some_and(|output| !output.matches_spec(decoded.spec()))
        {
            if let Some(output) = self.cpal_output.as_mut() {
                output.flush();
            }
            self.cpal_output = None;
        }
        if self.cpal_output.is_none() {
            let spec = decoded.spec().clone();
            let output = self
                .cpal_output_stream
                .as_ref()
                .context("output stream is unavailable")?
                .create_output(spec, duration)?;
            self.cpal_output = Some(output);
        }

        self.cpal_output.as_mut().unwrap().write(decoded);
        Ok(())
    }

    fn finish_playback(&mut self, skip_future_samples: bool) -> anyhow::Result<()> {
        if !skip_future_samples {
            if let Some(output) = self.cpal_output.as_mut() {
                output.flush();
            }
            self.wait_for_running_preload()?;
        } else {
            self.clear_output_buffer();
        }

        if let Some(playback) = self.preload_playback.take() {
            let progress = ProgressState {
                position: 0,
                duration: playback.duration,
            };
            self.playback = Some(playback);
            // Every track gets a fresh converter so a sample-rate or channel
            // change cannot reuse state configured for the previous track.
            self.cpal_output = None;
            let stats = self.controls.stats_handle();
            stats.clear_source_format();
            stats.set_source_buffer_signal(
                self.playback
                    .as_ref()
                    .map(|playback| Arc::clone(&playback.buffer_signal)),
            );
            self.controls.ready(progress);
            self.ensure_output_stream()?;
            self.controls.set_output_enabled(true);
            self.cpal_output_stream.as_ref().unwrap().play()?;
            self.state = DecoderState::Playing;
            self.controls.preload_played();
            self.controls.playing();
        } else if !skip_future_samples {
            // The remaining samples are an expected tail, not an underrun.
            self.controls.set_is_playing(false);
            self.drain_output_buffer();
            self.stop_internal(true);
        }

        Ok(())
    }

    fn wait_for_running_preload(&mut self) -> anyhow::Result<()> {
        if self.preload_thread.is_none() || self.preload_playback.is_some() {
            return Ok(());
        }

        let deadline = Instant::now() + self.config.preload.gapless_wait_timeout;
        while Instant::now() < deadline {
            self.poll_preload_thread()?;
            if self.preload_playback.is_some() || self.preload_thread.is_none() {
                break;
            }
            thread::sleep(Duration::from_millis(2));
        }
        Ok(())
    }

    fn stop_internal(&mut self, notify: bool) {
        self.pause_output();
        self.clear_output_buffer();
        self.state = DecoderState::Idle;
        self.cpal_output = None;
        self.playback = None;
        self.clear_preload();
        self.controls.set_seek_ts(None);
        self.controls.set_buffering_realtime(false);
        let stats = self.controls.stats_handle();
        stats.clear_source_format();
        stats.set_source_buffer_signal(None);
        if notify {
            self.controls.stopped();
        }
    }

    fn clear_preload(&mut self) {
        self.preload_generation = self.preload_generation.wrapping_add(1);
        if let Some(cancel) = self.preload_cancel.take() {
            cancel.store(true, std::sync::atomic::Ordering::Release);
        }
        self.preload_playback = None;
        self.preload_thread = None;
        self.controls.set_is_file_preloaded(false);
    }

    fn drain_output_buffer(&self) {
        let Some(output) = &self.cpal_output_stream else {
            return;
        };
        let deadline =
            Instant::now() + self.config.output.buffer_duration + Duration::from_millis(100);
        while !output.ring_buffer_reader.is_empty() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(1));
        }
    }

    fn publish_buffering_state(&mut self) {
        let buffering = self.controls.stats().is_buffering;
        if buffering != self.reported_buffering {
            self.reported_buffering = buffering;
            self.controls.notify_buffering(buffering);
        }
    }

    fn pause_output(&self) {
        self.controls.set_output_enabled(false);
        if let Some(output) = &self.cpal_output_stream {
            let _ = output.pause();
        }
    }

    fn clear_output_buffer(&self) {
        if let Some(output) = &self.cpal_output_stream {
            let skipped = output.clear();
            if skipped > 0 {
                self.controls.stats_handle().dropped_samples(skipped);
            }
        }
    }

    fn ensure_output_stream(&mut self) -> anyhow::Result<()> {
        if self.cpal_output_stream.is_none() {
            self.cpal_output_stream = Some(CpalOutputStream::new(
                &self.config.output,
                self.controls.clone(),
            )?);
        }
        Ok(())
    }

    fn open(
        source: Box<dyn AnniSource>,
        buffer_signal: Arc<AtomicBool>,
        config: &PlayerConfig,
    ) -> anyhow::Result<Playback> {
        let duration_hint = source.duration_hint();
        let mss = MediaSourceStream::new(source.into(), Default::default());
        let reader = default::get_probe()
            .probe(
                &Hint::new(),
                mss,
                FormatOptions::default(),
                MetadataOptions::default(),
            )
            .context("Failed to create format reader")?;

        let track = reader
            .default_track(TrackType::Audio)
            .context("There are no audio tracks in the source")?;
        let track_id = track.id;
        let codec_params = track
            .codec_params
            .as_ref()
            .and_then(|params| params.audio())
            .context("The audio track has no codec parameters")?;
        let decoder_options = AudioDecoderOptions::default()
            .gapless(config.decode.gapless)
            .verify(config.decode.verify);
        let decoder = CODEC_REGISTRY.make_audio_decoder(codec_params, &decoder_options)?;

        let timebase = track
            .time_base
            .or_else(|| codec_params.sample_rate.and_then(TimeBase::try_from_recip));
        let duration = timebase
            .zip(track.duration)
            .and_then(|(timebase, duration)| {
                Timestamp::try_from(duration.get())
                    .ok()
                    .map(|timestamp| (timebase, timestamp))
            })
            .and_then(|(timebase, timestamp)| timebase.calc_time(timestamp))
            .and_then(|time| u64::try_from(time.as_millis()).ok())
            .unwrap_or_else(|| {
                duration_hint
                    .map(|duration| duration.saturating_mul(1000))
                    .unwrap_or(0)
            });

        Ok(Playback {
            reader,
            decoder,
            track_id,
            timebase,
            duration,
            buffer_signal,
            preload: VecDeque::new(),
            seek_target: None,
            consecutive_decode_errors: 0,
        })
    }

    fn preload(
        source: Box<dyn AnniSource>,
        buffer_signal: Arc<AtomicBool>,
        config: PlayerConfig,
        cancel: Arc<AtomicBool>,
        stats: crate::stats::PlayerStatsHandle,
        controls: Controls,
        generation: u64,
    ) -> JoinHandle<anyhow::Result<Playback>> {
        thread::spawn(move || {
            let result = (|| {
                let mut playback = Self::open(source, buffer_signal, &config)?;
                let target_ms = config.preload.target_duration.as_millis() as u64;
                let mut decoded_ms = 0;
                let mut consecutive_decode_errors = 0;

                'preload: for _ in 0..config.preload.max_packets {
                    if cancel.load(std::sync::atomic::Ordering::Acquire) {
                        return Err(anyhow!("preload was cancelled"));
                    }
                    let packet = loop {
                        match playback.reader.next_packet()? {
                            Some(packet) if packet.track_id == playback.track_id => break packet,
                            Some(_) => continue,
                            None => break 'preload,
                        }
                    };
                    let position = timestamp_to_millis(playback.timebase, packet.pts);
                    let decoded = match playback.decoder.decode(&packet) {
                        Ok(decoded) => {
                            consecutive_decode_errors = 0;
                            decoded
                        }
                        Err(_) if config.decode.recover_decode_errors => {
                            consecutive_decode_errors += 1;
                            if consecutive_decode_errors > config.decode.max_consecutive_errors {
                                return Err(anyhow!("too many consecutive preload decode errors"));
                            }
                            stats.recoverable_decode_error();
                            continue;
                        }
                        Err(error) => return Err(error.into()),
                    };
                    let spec = decoded.spec().clone();
                    let frames = decoded.frames();
                    stats.preloaded(frames);
                    let mut buffer = AudioBuffer::new(spec.clone(), decoded.capacity());
                    buffer.resize_uninit(frames);
                    decoded.copy_to(&mut buffer);
                    playback
                        .preload
                        .push_back(PreloadedPacket { buffer, position });
                    decoded_ms += frames as u64 * 1000 / u64::from(spec.rate());

                    if decoded_ms >= target_ms {
                        break;
                    }
                }

                if playback.preload.is_empty() {
                    return Err(anyhow!("cannot preload an empty audio source"));
                }
                Ok(playback)
            })();
            controls.send_internal_event(InternalPlayerEvent::PreloadFinished(generation));
            result
        })
    }

    fn poll_preload_thread(&mut self) -> anyhow::Result<()> {
        if self
            .preload_thread
            .as_ref()
            .is_none_or(|thread| !thread.is_finished())
        {
            return Ok(());
        }

        self.complete_preload_thread()
    }

    fn complete_preload_thread(&mut self) -> anyhow::Result<()> {
        let Some(handle) = self.preload_thread.take() else {
            return Ok(());
        };
        self.preload_cancel = None;
        let playback = handle
            .join()
            .unwrap_or_else(|_| Err(anyhow!("could not join preload thread")))?;
        self.preload_playback = Some(playback);
        self.controls.set_is_file_preloaded(true);
        self.controls.preload_ready();
        Ok(())
    }
}

fn timestamp_to_millis(timebase: Option<TimeBase>, timestamp: Timestamp) -> u64 {
    timebase
        .and_then(|timebase| timebase.calc_time(timestamp))
        .and_then(|time| u64::try_from(time.as_millis()).ok())
        .unwrap_or(0)
}

fn frames_between_timestamps(
    timebase: Option<TimeBase>,
    start: Timestamp,
    end: Timestamp,
    sample_rate: u32,
) -> usize {
    let Some(timebase) = timebase else {
        return 0;
    };
    let Some(ticks) = end.duration_from(start) else {
        return 0;
    };
    let numerator =
        u128::from(ticks.get()) * u128::from(timebase.numer.get()) * u128::from(sample_rate);
    numerator.div_ceil(u128::from(timebase.denom.get())) as usize
}

enum DecoderState {
    Playing,
    Paused,
    Idle,
}

impl DecoderState {
    fn is_playing(&self) -> bool {
        matches!(self, Self::Playing)
    }

    fn is_paused(&self) -> bool {
        matches!(self, Self::Paused)
    }

    fn is_idle(&self) -> bool {
        matches!(self, Self::Idle)
    }
}

struct Playback {
    reader: Box<dyn FormatReader>,
    track_id: u32,
    decoder: Box<dyn AudioDecoder>,
    timebase: Option<TimeBase>,
    duration: u64,
    buffer_signal: Arc<AtomicBool>,
    preload: VecDeque<PreloadedPacket>,
    seek_target: Option<Timestamp>,
    consecutive_decode_errors: usize,
}

struct PreloadedPacket {
    buffer: AudioBuffer<f32>,
    position: u64,
}

#[cfg(test)]
mod tests {
    use std::{
        fs::File,
        num::NonZeroU32,
        sync::{atomic::AtomicBool, Arc},
        time::Duration,
    };

    use symphonia::core::units::{TimeBase, Timestamp};

    use super::{frames_between_timestamps, Decoder};
    use crate::{
        config::PlayerConfig,
        stats::PlayerStatsHandle,
        types::{InternalPlayerEvent, PlayerEvent},
        Controls,
    };

    fn fixture() -> File {
        File::open(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets/1s.flac"))
            .unwrap()
    }

    #[test]
    fn opens_audio_without_requiring_an_output_device() {
        let playback = Decoder::open(
            Box::new(fixture()),
            Arc::new(AtomicBool::new(false)),
            &PlayerConfig::default(),
        )
        .unwrap();

        assert_eq!(playback.duration, 1_000);
    }

    #[test]
    fn preloads_more_than_a_single_packet() {
        let (sender, _receiver) = std::sync::mpsc::channel();
        let controls = Controls::new(sender);
        let wakeups = controls.event_handler().1.clone();
        let preload = Decoder::preload(
            Box::new(fixture()),
            Arc::new(AtomicBool::new(false)),
            PlayerConfig::default(),
            Arc::new(AtomicBool::new(false)),
            PlayerStatsHandle::default(),
            controls,
            1,
        );
        assert!(matches!(
            wakeups.recv_timeout(Duration::from_secs(1)),
            Ok(InternalPlayerEvent::PreloadFinished(1))
        ));
        let playback = preload.join().unwrap().unwrap();

        assert!(playback.preload.len() > 1);
    }

    #[test]
    fn accurate_seek_converts_timestamp_delta_to_frames() {
        let timebase = TimeBase::new(NonZeroU32::new(1).unwrap(), NonZeroU32::new(1_000).unwrap());
        assert_eq!(
            frames_between_timestamps(
                Some(timebase),
                Timestamp::new(1_000),
                Timestamp::new(1_125),
                48_000,
            ),
            6_000,
        );
    }

    #[test]
    fn disconnected_legacy_killer_does_not_terminate_decoder() {
        let (event_sender, events) = std::sync::mpsc::channel();
        let controls = Controls::new(event_sender);
        let (killer, killer_receiver) = crossbeam::channel::unbounded::<bool>();
        drop(killer);
        let decoder =
            Decoder::with_config_lazy(controls.clone(), PlayerConfig::default(), killer_receiver);
        let thread = std::thread::spawn(move || decoder.start());

        controls.open(Box::new(fixture()), Arc::new(AtomicBool::new(false)), false);
        assert!(matches!(
            events.recv_timeout(Duration::from_secs(1)),
            Ok(PlayerEvent::Progress(_)) | Ok(PlayerEvent::Ready(_))
        ));
        // Ready may be preceded by Progress, so wait for the confirmed state.
        while !matches!(controls.stats().status, crate::PlaybackStatus::Ready) {
            let _ = events.recv_timeout(Duration::from_secs(1)).unwrap();
        }

        controls.shutdown();
        thread.join().unwrap();
    }
}
