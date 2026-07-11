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
    sync::{atomic::AtomicBool, Arc},
    thread::{self, JoinHandle},
};

use anyhow::{anyhow, Context};
use once_cell::sync::Lazy;
use symphonia::{
    core::{
        audio::{AsGenericAudioBufferRef, Audio, AudioBuffer, AudioSpec, Channels, Position},
        codecs::{
            audio::{AudioDecoder, AudioDecoderOptions},
            registry::CodecRegistry,
        },
        formats::{probe::Hint, FormatOptions, FormatReader, SeekMode, SeekTo, TrackType},
        io::MediaSourceStream,
        meta::MetadataOptions,
        units::{Time, TimeBase, Timestamp},
    },
    default::{self, register_enabled_codecs},
};

use super::opus::OpusDecoder;
use crate::{
    controls::*,
    cpal_output::{CpalOutput, CpalOutputStream},
    sources::AnniSource,
    types::*,
};

enum PlaybackState {
    Playing,
    Switched,
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
    controls: Controls,
    state: DecoderState,
    cpal_output_stream: CpalOutputStream,
    cpal_output: Option<CpalOutput>,
    playback: Option<Playback>,
    preload_playback: Option<Playback>,
    /// The `JoinHandle` for the thread that preloads a file.
    preload_thread: Option<JoinHandle<anyhow::Result<Playback>>>,
    spec: AudioSpec,
}

impl Decoder {
    /// Creates a new decoder.
    pub fn new(controls: Controls, sample_rate: u32, thread_killer: Receiver<bool>) -> Self {
        let spec = AudioSpec::new(
            sample_rate,
            Channels::from(Position::FRONT_LEFT | Position::FRONT_RIGHT),
        );

        Decoder {
            thread_killer,
            controls: controls.clone(),
            state: DecoderState::Idle,
            cpal_output_stream: CpalOutputStream::new(spec.clone(), controls).unwrap(),
            cpal_output: None,
            playback: None,
            preload_playback: None,
            preload_thread: None,
            spec,
        }
    }

    /// Starts decoding in an infinite loop.
    /// Listens for any incoming `ThreadMessage`s.
    ///
    /// If playing, then the decoder decodes packets
    /// until the file is done playing.
    ///
    /// If stopped, the decoder goes into an idle state
    /// where it waits for a message to come.
    pub fn start(mut self) {
        loop {
            // Check if the preload thread is done.
            if let Err(e) = self.poll_preload_thread() {
                log::error!("Decode error on poll_preload_thread: {e}");
            }

            // Check for incoming `ThreadMessage`s.
            match self.listen_for_message() {
                Ok(should_break) => {
                    if should_break {
                        break;
                    }
                }
                Err(e) => {
                    log::error!("Decode error on listen_for_message: {e}");
                }
            }

            // Decode and output the samples.
            match self.do_playback() {
                Ok(playback_state) => match playback_state {
                    PlaybackState::Switched => {
                        self.finish_playback(true);
                    }
                    PlaybackState::Completed => {
                        self.state = DecoderState::Idle;
                        self.finish_playback(false);
                    }
                    _ => (),
                },
                Err(e) => {
                    log::error!("Decode error on do_playback: {e}");
                }
            }
        }
    }

    /// Listens for any incoming messages.
    ///
    /// Blocks if the `self.state` is `Idle` or `Paused`.
    ///
    /// Returns true if this thread should be stopped.
    /// Returns false otherwise.
    fn listen_for_message(&mut self) -> anyhow::Result<bool> {
        if self.thread_killer.try_recv().is_ok() {
            return Ok(true);
        }

        // If the player is paused, then block this thread until a message comes in
        // to save the CPU.
        let recv: Option<InternalPlayerEvent> = if self.state.is_idle() || self.state.is_paused() {
            self.controls.event_handler().1.recv().ok()
        } else {
            self.controls.event_handler().1.try_recv().ok()
        };

        match recv {
            None => (),
            Some(message) => match message {
                InternalPlayerEvent::Open(source, buffer_signal) => {
                    let playback = Self::open(source, buffer_signal)?;
                    self.controls.set_progress(ProgressState {
                        position: 0,
                        duration: playback.duration,
                    });
                    self.cpal_output = None;
                    self.playback = Some(playback);
                }
                InternalPlayerEvent::Play => {
                    self.state = DecoderState::Playing;

                    if self.cpal_output.is_some() {
                        self.cpal_output_stream.play();
                    }
                }
                InternalPlayerEvent::Pause => {
                    self.state = DecoderState::Paused;

                    if self.cpal_output.is_some() {
                        self.cpal_output_stream.pause();
                    }
                }
                InternalPlayerEvent::Stop => {
                    self.state = DecoderState::Idle;
                    self.cpal_output = None;
                    self.playback = None;
                }
                // When the device is changed/disconnected,
                // then we should reestablish a connection.
                // To make a new connection, dispose of the current cpal_output
                // and pause playback. Once the user is ready, they can start
                // playback themselves.
                InternalPlayerEvent::DeviceChanged => {
                    log::debug!("device changed");
                    self.controls.pause();
                    self.cpal_output = None;
                }
                InternalPlayerEvent::Preload(source, buffer_signal) => {
                    self.preload_playback = None;
                    self.controls.set_is_file_preloaded(false);
                    let handle = self.preload(source, buffer_signal);
                    self.preload_thread = Some(handle);
                }
                InternalPlayerEvent::PlayPreloaded => {
                    self.finish_playback(true);
                }
            },
        }

        Ok(false)
    }

    /// Decodes a packet and writes to `cpal_output`.
    ///
    /// Returns `true` when the playback is complete.
    /// Returns `false` otherwise.
    fn do_playback(&mut self) -> anyhow::Result<PlaybackState> {
        // Nothing to do.
        if self.playback.is_none() || self.state.is_idle() || self.state.is_paused() {
            return Ok(PlaybackState::Idle);
        }

        let playback = self.playback.as_mut().unwrap();

        // If there is audio already decoded from preloading,
        // then output that instead.
        if let Some(preload) = playback.preload.take() {
            // Write the decoded packet to CPAL.
            if self.cpal_output.is_none() {
                let spec = preload.spec().clone();
                let duration = preload.capacity() as u64;

                self.cpal_output_stream =
                    CpalOutputStream::new(self.spec.clone(), self.controls.clone())?;
                self.cpal_output
                    .replace(self.cpal_output_stream.create_output(
                        playback.buffer_signal.clone(),
                        spec,
                        duration,
                    ));
            }

            let buffer_ref = preload.as_generic_audio_buffer_ref();
            self.cpal_output.as_mut().unwrap().write(buffer_ref);

            return Ok(PlaybackState::Playing);
        }

        if let Some(seek_ts) = *self.controls.seek_ts() {
            let seek_to = SeekTo::Time {
                time: Time::from_millis_u64(seek_ts),
                track_id: Some(playback.track_id),
            };
            playback.reader.seek(SeekMode::Coarse, seek_to)?;
        }

        // Clean up seek stuff.
        if self.controls.seek_ts().is_some() {
            self.controls.set_seek_ts(None);
            playback.decoder.reset();
            // Clear the ring buffer which prevents the writer
            // from blocking.
            if self.cpal_output.is_some() {
                self.cpal_output_stream.ring_buffer_reader.skip_all();
            }
            return Ok(PlaybackState::Playing);
        }

        // Decode the next packet.
        let packet = match playback.reader.next_packet() {
            Ok(Some(packet)) => packet,
            // An error occurs when the stream
            // has reached the end of the audio.
            Ok(None) | Err(_) => {
                if self.controls.is_looping() {
                    self.controls.set_seek_ts(Some(0));
                    // crate::utils::callback_stream::update_callback_stream(Callback::PlaybackLooped);
                    return Ok(PlaybackState::Playing);
                }

                return Ok(PlaybackState::Completed);
            }
        };

        if packet.track_id != playback.track_id {
            return Ok(PlaybackState::Switched);
        }

        let decoded = playback
            .decoder
            .decode(&packet)
            .context("Could not decode audio packet.")?;

        let position = playback
            .timebase
            .and_then(|timebase| timebase.calc_time(packet.pts))
            .and_then(|time| u64::try_from(time.as_millis()).ok())
            .unwrap_or(0);

        // Update the progress stream with calculated times.
        let progress = ProgressState {
            position,
            duration: playback.duration,
        };

        self.controls.set_progress(progress);

        // Write the decoded packet to CPAL.
        if self.cpal_output.is_none() {
            let spec = decoded.spec().clone();
            let duration = decoded.capacity() as u64;
            self.cpal_output_stream =
                CpalOutputStream::new(self.spec.clone(), self.controls.clone())?;
            self.cpal_output
                .replace(self.cpal_output_stream.create_output(
                    playback.buffer_signal.clone(),
                    spec,
                    duration,
                ));
        }

        self.cpal_output.as_mut().unwrap().write(decoded);

        Ok(PlaybackState::Playing)
    }

    /// Called when the file is finished playing.
    ///
    /// Flushes `cpal_output` and sends a `Done` message to Dart.
    fn finish_playback(&mut self, skip_future_samples: bool) {
        if !skip_future_samples {
            if let Some(cpal_output) = self.cpal_output.as_mut() {
                // There may be samples left over and we don't want to
                // start playing another file before they are read.
                cpal_output.flush();
            }
        } else {
            self.cpal_output_stream.ring_buffer_reader.skip_all();
        }

        // If there is a preloaded file, then swap it with the current playback.
        if let Some(playback) = self.preload_playback.take() {
            self.playback = Some(playback);

            self.controls.send_internal_event(InternalPlayerEvent::Play);
            self.controls.preload_played();
        } else if !skip_future_samples {
            // do not skip future samples, which means the track has finished playing without user interaction
            // stop the playback, and send `Stop` event
            self.controls.stop();
        }
    }

    /// Opens the given source for playback. Returns a `Playback`
    /// for the source.
    fn open(
        source: Box<dyn AnniSource>,
        buffer_signal: Arc<AtomicBool>,
    ) -> anyhow::Result<Playback> {
        let duration_hint = source.duration_hint();
        let mss = MediaSourceStream::new(source.into(), Default::default());
        let format_options = FormatOptions::default();
        let metadata_options: MetadataOptions = Default::default();

        let reader = default::get_probe()
            .probe(&Hint::new(), mss, format_options, metadata_options)
            .context("Failed to create format reader.")?;

        let track = reader
            .default_track(TrackType::Audio)
            .context("Cannot start playback. There are no tracks present in the file.")?;
        let track_id = track.id;
        let codec_params = track
            .codec_params
            .as_ref()
            .and_then(|params| params.audio())
            .context("Cannot start playback. The audio track has no codec parameters.")?;

        let decoder =
            CODEC_REGISTRY.make_audio_decoder(codec_params, &AudioDecoderOptions::default())?;

        // Used only for outputting the current position and duration.
        let timebase = track
            .time_base
            .or_else(|| codec_params.sample_rate.and_then(TimeBase::try_from_recip));
        let duration = timebase
            .zip(track.duration)
            .and_then(|(timebase, duration)| {
                Timestamp::try_from(duration.get())
                    .ok()
                    .map(|ts| (timebase, ts))
            })
            .and_then(|(timebase, ts)| timebase.calc_time(ts))
            .and_then(|time| u64::try_from(time.as_millis()).ok())
            .unwrap_or_else(|| duration_hint.map(|dur| dur * 1000).unwrap_or(0));

        Ok(Playback {
            reader,
            decoder,
            track_id,
            timebase,
            duration,
            buffer_signal,
            preload: None,
        })
    }

    /// Spawns a thread that decodes the first packet of the source.
    ///
    /// Returns a preloaded `Playback` and `CpalOutput` when complete.
    fn preload(
        &self,
        source: Box<dyn AnniSource>,
        buffer_signal: Arc<AtomicBool>,
    ) -> JoinHandle<anyhow::Result<Playback>> {
        thread::spawn(move || {
            let mut playback = Self::open(source, buffer_signal.clone())?;
            // Preload
            let packet = playback
                .reader
                .next_packet()?
                .context("Cannot preload an empty audio source.")?;
            let buf_ref = playback.decoder.decode(&packet)?;

            let spec = buf_ref.spec().clone();
            let capacity = buf_ref.capacity();

            let mut buf = AudioBuffer::new(spec, capacity);
            buf.resize_uninit(buf_ref.frames());
            buf_ref.copy_to(&mut buf);
            playback.preload = Some(buf);

            Ok(playback)
        })
    }

    /// Polls the `preload_thread`.
    ///
    /// If it is finished, the preloaded file
    /// is then placed in `preload_playback`.
    fn poll_preload_thread(&mut self) -> anyhow::Result<()> {
        if self.preload_thread.is_none() || !self.preload_thread.as_ref().unwrap().is_finished() {
            return Ok(());
        }

        let handle = self.preload_thread.take().unwrap();
        let result = handle
            .join()
            .unwrap_or(Err(anyhow!("Could not join preload thread.")))?;

        self.preload_playback.replace(result);
        self.controls.set_is_file_preloaded(true);

        Ok(())
    }
}

enum DecoderState {
    Playing,
    Paused,
    Idle,
}

impl DecoderState {
    fn is_idle(&self) -> bool {
        if let DecoderState::Idle = self {
            return true;
        }

        false
    }

    fn is_paused(&self) -> bool {
        if let DecoderState::Paused = self {
            return true;
        }

        false
    }
}

/// Holds the items related to playback.
///
/// Ex: The Symphonia decoder, timebase, duration.
struct Playback {
    reader: Box<dyn FormatReader>,
    track_id: u32,
    decoder: Box<dyn AudioDecoder>,
    timebase: Option<TimeBase>,
    duration: u64,
    buffer_signal: Arc<AtomicBool>,
    /// A buffer of already decoded samples.
    preload: Option<AudioBuffer<f32>>,
}
