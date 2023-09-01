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

use anyhow::Context;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, Stream, StreamConfig,
};
use symphonia::core::audio::{AudioBufferRef, SampleBuffer, SignalSpec};

use crate::types::InternalPlayerEvent;
use crate::utils::blocking_rb::*;

use super::{
    controls::*,
    dsp::{normalizer::Normalizer, resampler::Resampler},
};

/// The default output volume is way too high.
/// Multiplying the volume input by this number
/// will help to reduce it.
const BASE_VOLUME: f32 = 0.8;

pub struct CpalOutputStream {
    pub stream: Stream,
    pub ring_buffer_reader: BlockingRb<f32, Consumer>,
    pub ring_buffer_writer: BlockingRb<f32, Producer>,
    pub device: Device,
    pub config: StreamConfig,
    //
    controls: Controls,
}

impl CpalOutputStream {
    pub fn new(spec: SignalSpec, controls: Controls) -> anyhow::Result<Self> {
        // Get the output config.
        let (device, config) = Self::get_config(spec)?;

        // Create a ring buffer with a capacity for up-to `buf_len_ms` of audio.
        let channels = spec.channels.count();
        let buf_len_ms = 300;
        let ring_len = ((buf_len_ms * spec.rate as usize) / 1000) * channels;

        // Create the buffers for the stream.
        let rb = BlockingRb::<f32>::new(ring_len);
        let rb_clone = rb.clone();
        let ring_buffer_writer = rb.0;
        let ring_buffer_reader = rb.1;

        let stream = device.build_output_stream(
            &config,
            {
                let controls = controls.clone();
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    // let buffering = buffer_signal.load(std::sync::atomic::Ordering::SeqCst);

                    // // "Pause" the stream.
                    // // What this really does is mute the stream.
                    // // With only a return statement, the current sample still plays.
                    // // CPAL states that `stream.pause()` may not work for all devices.
                    // // `stream.pause()` is the ideal way to play/pause.
                    // if (cfg!(target_os = "windows")
                    //     && !controls.is_playing()
                    //     && !controls.is_stopped())
                    //     || buffering
                    // {
                    //     data.iter_mut().for_each(|s| *s = 0.0);

                    //     if buffering {
                    //         ring_buffer_reader.skip_all();
                    //     }

                    //     return;
                    // }

                    // Set the volume.
                    // TODO: allow user to not normalize the volume.
                    if let Some(written) = ring_buffer_reader.read(data) {
                        data[0..written]
                            .iter_mut()
                            .for_each(|s| *s *= BASE_VOLUME * *controls.volume());
                    }
                }
            },
            {
                let controls = controls.clone();
                move |err| {
                    match err {
                        cpal::StreamError::DeviceNotAvailable => {
                            // Tell the decoder that there is no longer a valid device.
                            // The decoder will make a new `cpal_output`.
                            controls.send_internal_event(InternalPlayerEvent::DeviceChanged);
                            ring_buffer_writer.cancel_write();
                        }
                        cpal::StreamError::BackendSpecific { err } => {
                            // This should never happen.
                            panic!("Unknown error occurred during playback: {err}");
                        }
                    }
                }
            },
            None,
        );

        let stream = stream.context("Could not build the stream.")?;
        stream.play()?;

        Ok(Self {
            stream,
            device,
            config,
            ring_buffer_writer: rb_clone.0,
            ring_buffer_reader: rb_clone.1,
            controls,
        })
    }

    /// Starts a new stream on the default device.
    pub fn create_output(
        &self,
        buffer_signal: Arc<AtomicBool>,
        spec: SignalSpec,
        duration: u64,
    ) -> CpalOutput {
        CpalOutput::new(
            buffer_signal,
            spec,
            duration,
            self.config.clone(),
            self.controls.clone(),
            self.ring_buffer_writer.clone(),
        )
    }

    pub fn play(&self) {
        if self.stream.play().is_err() {
            // TODO: stream play is not supported, use another way to play
        }
    }

    pub fn pause(&self) {
        if self.stream.pause().is_err() {
            // TODO: stream pause is not supported, use another way to pause
        }
    }

    fn get_config(spec: SignalSpec) -> anyhow::Result<(Device, StreamConfig)> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .context("Failed to get default output device.")?;

        let config;

        #[cfg(target_os = "windows")]
        {
            let mut supported_configs = device
                .supported_output_configs()
                .context("Failed to get supported output configs.")?;
            config = supported_configs
                .next()
                .context("Failed to get a config.")?
                .with_max_sample_rate()
                .config();
        }

        #[cfg(not(target_os = "windows"))]
        {
            let channels = spec.channels.count();
            config = cpal::StreamConfig {
                channels: channels as cpal::ChannelCount,
                sample_rate: cpal::SampleRate(spec.rate),
                buffer_size: cpal::BufferSize::Default,
            };
        }

        Ok((device, config))
    }
}

//TODO: Support i16 and u16 instead of only f32.
pub struct CpalOutput {
    pub spec: SignalSpec,
    pub duration: u64,
    pub buffer_signal: Arc<AtomicBool>,
    ring_buffer_writer: BlockingRb<f32, Producer>,
    sample_buffer: SampleBuffer<f32>,
    resampler: Option<Resampler<f32>>,
    normalizer: Normalizer,
    controls: Controls,
}

impl CpalOutput {
    pub fn new(
        buffer_signal: Arc<AtomicBool>,
        spec: SignalSpec,
        duration: u64,
        config: StreamConfig,
        controls: Controls,
        ring_buffer_writer: BlockingRb<f32, Producer>,
    ) -> Self {
        // Create a resampler only if the code is running on Windows
        // or if the output config's sample rate doesn't match the audio's.
        let resampler: Option<Resampler<f32>> =
            if cfg!(target_os = "windows") || spec.rate != config.sample_rate.0 {
                Some(Resampler::new(
                    spec,
                    config.sample_rate.0 as usize,
                    duration,
                ))
            } else {
                None
            };

        let sample_buffer = SampleBuffer::<f32>::new(duration, spec);
        let sample_rate = config.sample_rate.0;

        Self {
            spec,
            duration,
            buffer_signal,
            ring_buffer_writer,
            sample_buffer,
            resampler,
            normalizer: Normalizer::new(spec.channels.count(), sample_rate),
            controls: controls,
        }
    }

    /// Write the `AudioBufferRef` to the buffers.
    pub fn write(&mut self, decoded: AudioBufferRef) {
        if decoded.frames() == 0 {
            return;
        }

        let mut samples = if let Some(resampler) = &mut self.resampler {
            // If there is a resampler, then write resampled values
            // instead of the normal `samples`.
            resampler.resample(decoded).unwrap_or(&[])
        } else {
            self.sample_buffer.copy_interleaved_ref(decoded);
            self.sample_buffer.samples()
        };

        if self.controls.is_normalizing() {
            if let Some(normalized) = self.normalizer.normalize(samples) {
                samples = normalized;
            }
        }

        while let Some(written) = self.ring_buffer_writer.write(samples) {
            samples = &samples[written..];
        }
    }

    /// Clean up after playback is done.
    pub fn flush(&mut self) {
        // If there is a resampler, then it may need to be flushed
        // depending on the number of samples it has.
        if let Some(resampler) = &mut self.resampler {
            let mut remaining_samples = resampler.flush().unwrap_or_default();

            while let Some(written) = self.ring_buffer_writer.write(remaining_samples) {
                remaining_samples = &remaining_samples[written..];
            }
        }
    }
}

unsafe impl Send for CpalOutput {}
