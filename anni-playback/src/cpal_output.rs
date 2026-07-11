// This file is a part of simple_audio
// Copyright (c) 2022-2023 Erikas Taroza <erikastaroza@gmail.com>

use anyhow::{anyhow, Context};
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, ErrorKind, FromSample, SampleFormat, SizedSample, Stream, StreamConfig,
    SupportedStreamConfig, I24, U24,
};
use symphonia::core::audio::{AudioSpec, Channels, GenericAudioBufferRef, Position};

use crate::{
    config::OutputSettings,
    controls::Controls,
    dsp::{normalizer::Normalizer, resampler::Resampler},
    stats::PlayerStatsHandle,
    types::{InternalPlayerEvent, PlaybackError, PlaybackErrorKind},
    utils::blocking_rb::{BlockingRb, Consumer, Producer},
};

const BASE_VOLUME: f32 = 0.8;

pub struct CpalOutputStream {
    pub stream: Stream,
    pub ring_buffer_reader: BlockingRb<f32, Consumer>,
    pub ring_buffer_writer: BlockingRb<f32, Producer>,
    pub config: StreamConfig,
    controls: Controls,
}

impl CpalOutputStream {
    pub fn new(settings: &OutputSettings, controls: Controls) -> anyhow::Result<Self> {
        let (device, supported_config) = Self::get_config(settings)?;
        let config = supported_config.config();
        let sample_format = supported_config.sample_format();
        let buffer_frames = duration_to_frames(settings.buffer_duration, config.sample_rate);
        let ring_len = buffer_frames
            .saturating_mul(config.channels as usize)
            .max(1);
        let (writer, reader) = BlockingRb::<f32>::new(ring_len);
        let stats = controls.stats_handle();

        let stream = match sample_format {
            SampleFormat::F32 => build_stream::<f32>(
                &device,
                config,
                reader.clone(),
                writer.clone(),
                controls.clone(),
                stats,
            ),
            SampleFormat::F64 => build_stream::<f64>(
                &device,
                config,
                reader.clone(),
                writer.clone(),
                controls.clone(),
                stats,
            ),
            SampleFormat::I8 => build_stream::<i8>(
                &device,
                config,
                reader.clone(),
                writer.clone(),
                controls.clone(),
                stats,
            ),
            SampleFormat::I16 => build_stream::<i16>(
                &device,
                config,
                reader.clone(),
                writer.clone(),
                controls.clone(),
                stats,
            ),
            SampleFormat::I24 => build_stream::<I24>(
                &device,
                config,
                reader.clone(),
                writer.clone(),
                controls.clone(),
                stats,
            ),
            SampleFormat::I32 => build_stream::<i32>(
                &device,
                config,
                reader.clone(),
                writer.clone(),
                controls.clone(),
                stats,
            ),
            SampleFormat::I64 => build_stream::<i64>(
                &device,
                config,
                reader.clone(),
                writer.clone(),
                controls.clone(),
                stats,
            ),
            SampleFormat::U8 => build_stream::<u8>(
                &device,
                config,
                reader.clone(),
                writer.clone(),
                controls.clone(),
                stats,
            ),
            SampleFormat::U16 => build_stream::<u16>(
                &device,
                config,
                reader.clone(),
                writer.clone(),
                controls.clone(),
                stats,
            ),
            SampleFormat::U24 => build_stream::<U24>(
                &device,
                config,
                reader.clone(),
                writer.clone(),
                controls.clone(),
                stats,
            ),
            SampleFormat::U32 => build_stream::<u32>(
                &device,
                config,
                reader.clone(),
                writer.clone(),
                controls.clone(),
                stats,
            ),
            SampleFormat::U64 => build_stream::<u64>(
                &device,
                config,
                reader.clone(),
                writer.clone(),
                controls.clone(),
                stats,
            ),
            format => Err(anyhow!("unsupported output sample format: {format}")),
        }
        .context("Could not build the output stream.")?;
        let stats = controls.stats_handle();
        stats.set_output_format(config.sample_rate, config.channels);
        stats.set_buffer(0, ring_len);

        Ok(Self {
            stream,
            config,
            ring_buffer_writer: writer,
            ring_buffer_reader: reader,
            controls,
        })
    }

    pub fn create_output(&self, spec: AudioSpec, duration: u64) -> anyhow::Result<CpalOutput> {
        CpalOutput::new(
            spec,
            duration,
            self.config,
            self.controls.clone(),
            self.ring_buffer_writer.clone(),
        )
    }

    pub fn play(&self) -> anyhow::Result<()> {
        self.stream.play().context("Could not start output stream")
    }

    pub fn pause(&self) -> anyhow::Result<()> {
        self.stream.pause().context("Could not pause output stream")
    }

    pub fn clear(&self) -> usize {
        let skipped = self.ring_buffer_reader.skip_all();
        self.controls.stats_handle().set_buffer(
            self.ring_buffer_reader.len(),
            self.ring_buffer_reader.capacity(),
        );
        skipped
    }

    fn get_config(settings: &OutputSettings) -> anyhow::Result<(Device, SupportedStreamConfig)> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .context("Failed to get default output device")?;
        log::debug!("default output device: {:?}", device.description());

        let default = match device.default_output_config() {
            Ok(config) => Some(config),
            Err(error) => {
                log::warn!("failed to get the default output config: {error}");
                None
            }
        };
        if settings.preferred_sample_rate.is_none()
            && settings.preferred_channels.is_none()
            && let Some(default) = default
            && is_supported_pcm_format(default.sample_format())
        {
            return Ok((device, default));
        }

        let all = match device.supported_output_configs() {
            Ok(configs) => configs
                .filter(|range| is_supported_pcm_format(range.sample_format()))
                .collect::<Vec<_>>(),
            Err(error) => {
                if let Some(default) = default {
                    log::warn!(
                        "failed to enumerate output configs ({error}); using the default config"
                    );
                    return Ok((device, default));
                }
                return Err(anyhow!(
                    "failed to enumerate output configs after the default config was unavailable: {error}"
                ));
            }
        };

        let mut candidates = all
            .iter()
            .copied()
            .filter(|range| {
                settings
                    .preferred_channels
                    .is_none_or(|channels| range.channels() == channels)
            })
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            candidates = all;
        }

        if let Some(rate) = settings.preferred_sample_rate {
            let matching = candidates
                .iter()
                .copied()
                .filter(|range| range.contains_rate(rate))
                .collect::<Vec<_>>();
            if !matching.is_empty() {
                candidates = matching;
            }
        }

        let range = candidates
            .into_iter()
            .max_by(|left, right| left.cmp_default_heuristics(right));
        let Some(range) = range else {
            return default
                .map(|default| (device, default))
                .context("the output device exposes no supported PCM config");
        };

        let selected = settings
            .preferred_sample_rate
            .and_then(|rate| range.try_with_sample_rate(rate))
            .or_else(|| range.try_with_standard_sample_rate())
            .unwrap_or_else(|| range.with_max_sample_rate());

        Ok((device, selected))
    }
}

impl Drop for CpalOutputStream {
    fn drop(&mut self) {
        self.controls.set_output_enabled(false);
        let stats = self.controls.stats_handle();
        stats.set_buffer(0, 0);
        stats.clear_output_format();
    }
}

fn is_supported_pcm_format(format: SampleFormat) -> bool {
    matches!(
        format,
        SampleFormat::F32
            | SampleFormat::F64
            | SampleFormat::I8
            | SampleFormat::I16
            | SampleFormat::I24
            | SampleFormat::I32
            | SampleFormat::I64
            | SampleFormat::U8
            | SampleFormat::U16
            | SampleFormat::U24
            | SampleFormat::U32
            | SampleFormat::U64
    )
}

fn build_stream<T>(
    device: &Device,
    config: StreamConfig,
    reader: BlockingRb<f32, Consumer>,
    writer: BlockingRb<f32, Producer>,
    controls: Controls,
    stats: PlayerStatsHandle,
) -> anyhow::Result<Stream>
where
    T: SizedSample + FromSample<f32>,
{
    let data_controls = controls.clone();
    let data_stats = stats.clone();
    let error_controls = controls.clone();

    let stream = device.build_output_stream(
        config,
        move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
            if !data_controls.output_enabled() {
                data.fill(T::EQUILIBRIUM);
                data_controls.set_buffering_realtime(false);
                data_stats.set_buffer(reader.len(), reader.capacity());
                return;
            }

            let volume = BASE_VOLUME * data_controls.volume_value();
            let written = reader.drain_with(data.len(), |index, sample| {
                data[index] = T::from_sample((sample * volume).clamp(-1.0, 1.0));
            });
            data[written..].fill(T::EQUILIBRIUM);

            if written < data.len() && data_controls.is_playing() {
                data_stats.underrun();
                data_controls.set_buffering_realtime(true);
            } else if written > 0 {
                data_controls.set_buffering_realtime(false);
            }

            data_stats.output_samples(written);
            data_stats.set_buffer(reader.len(), reader.capacity());
        },
        move |error| {
            let recoverable = matches!(
                error.kind(),
                ErrorKind::DeviceChanged
                    | ErrorKind::DeviceNotAvailable
                    | ErrorKind::StreamInvalidated
                    | ErrorKind::Xrun
                    | ErrorKind::RealtimeDenied
            );
            error_controls.report_error(PlaybackError::new(
                PlaybackErrorKind::Output,
                error.to_string(),
                !recoverable,
            ));

            if matches!(
                error.kind(),
                ErrorKind::DeviceChanged
                    | ErrorKind::DeviceNotAvailable
                    | ErrorKind::StreamInvalidated
            ) {
                error_controls.send_internal_event(InternalPlayerEvent::DeviceChanged);
                writer.cancel_write();
            } else if !recoverable {
                error_controls.send_internal_event(InternalPlayerEvent::OutputFailed);
                writer.cancel_write();
            }
        },
        None,
    )?;

    Ok(stream)
}

pub struct CpalOutput {
    spec: AudioSpec,
    output_channels: usize,
    ring_buffer_writer: BlockingRb<f32, Producer>,
    sample_buffer: Vec<f32>,
    mixed_buffer: Vec<f32>,
    resampler: Option<Resampler>,
    normalizer: Normalizer,
    controls: Controls,
}

impl CpalOutput {
    fn new(
        spec: AudioSpec,
        duration: u64,
        config: StreamConfig,
        controls: Controls,
        ring_buffer_writer: BlockingRb<f32, Producer>,
    ) -> anyhow::Result<Self> {
        let resampler = if spec.rate() != config.sample_rate {
            Some(Resampler::new(
                spec.clone(),
                config.sample_rate as usize,
                duration as usize,
            )?)
        } else {
            None
        };
        let output_channels = config.channels as usize;
        controls
            .stats_handle()
            .set_source_format(spec.rate(), spec.channels().count() as u16);

        Ok(Self {
            spec,
            output_channels,
            ring_buffer_writer,
            sample_buffer: Vec::new(),
            mixed_buffer: Vec::new(),
            resampler,
            normalizer: Normalizer::new(output_channels, config.sample_rate),
            controls,
        })
    }

    pub fn matches_spec(&self, spec: &AudioSpec) -> bool {
        self.spec.rate() == spec.rate() && self.spec.channels() == spec.channels()
    }

    pub fn write(&mut self, decoded: GenericAudioBufferRef<'_>) {
        if decoded.frames() == 0 {
            return;
        }

        let input_channels = decoded.spec().channels().clone();
        let source_samples = if let Some(resampler) = &mut self.resampler {
            let Some(samples) = resampler.resample(decoded) else {
                return;
            };
            samples
        } else {
            self.sample_buffer.clear();
            decoded.copy_to_vec_interleaved(&mut self.sample_buffer);
            &self.sample_buffer
        };

        remix_interleaved(
            source_samples,
            &input_channels,
            self.output_channels,
            &mut self.mixed_buffer,
        );
        write_mixed(
            &mut self.mixed_buffer,
            &mut self.normalizer,
            &self.controls,
            &self.ring_buffer_writer,
        );
    }

    pub fn flush(&mut self) {
        let Some(resampler) = &mut self.resampler else {
            return;
        };
        let input_channels = self.spec.channels().clone();
        let Some(samples) = resampler.flush() else {
            return;
        };

        remix_interleaved(
            samples,
            &input_channels,
            self.output_channels,
            &mut self.mixed_buffer,
        );
        write_mixed(
            &mut self.mixed_buffer,
            &mut self.normalizer,
            &self.controls,
            &self.ring_buffer_writer,
        );
    }
}

fn write_mixed(
    samples: &mut [f32],
    normalizer: &mut Normalizer,
    controls: &Controls,
    writer: &BlockingRb<f32, Producer>,
) {
    let samples = if controls.is_normalizing() {
        normalizer.normalize(samples).unwrap_or(samples)
    } else {
        samples
    };

    let mut remaining = samples;
    while !remaining.is_empty() {
        let Some(written) = writer.write(remaining) else {
            controls.stats_handle().dropped_samples(remaining.len());
            break;
        };
        remaining = &remaining[written..];
    }
    controls
        .stats_handle()
        .set_buffer(writer.len(), writer.capacity());
}

fn remix_interleaved(
    input: &[f32],
    input_channels: &Channels,
    out_channels: usize,
    output: &mut Vec<f32>,
) {
    output.clear();
    let in_channels = input_channels.count();
    if in_channels == 0 || out_channels == 0 {
        return;
    }

    let frames = input.len() / in_channels;
    output.reserve(frames.saturating_mul(out_channels));
    for frame in input.chunks_exact(in_channels) {
        match (in_channels, out_channels) {
            (1, 1) => output.push(frame[0]),
            (1, 2) => output.extend([frame[0], frame[0]]),
            (1, channels) => {
                output.extend([frame[0], frame[0]]);
                output.extend(std::iter::repeat_n(0.0, channels.saturating_sub(2)));
            }
            (_, 1) => {
                let (left, right) = stereo_downmix(frame, input_channels);
                output.push(((left + right) * 0.5).clamp(-1.0, 1.0));
            }
            (_, 2) => {
                let (left, right) = stereo_downmix(frame, input_channels);
                output.extend([left.clamp(-1.0, 1.0), right.clamp(-1.0, 1.0)]);
            }
            (same_in, same_out) if same_in == same_out => output.extend_from_slice(frame),
            _ => {
                for channel in 0..out_channels {
                    output.push(frame.get(channel).copied().unwrap_or(0.0));
                }
            }
        }
    }
}

fn stereo_downmix(frame: &[f32], channels: &Channels) -> (f32, f32) {
    let Channels::Positioned(positions) = channels else {
        let mut left = frame[0];
        let mut right = frame.get(1).copied().unwrap_or(left);
        if frame.len() > 2 {
            let extra = frame[2..].iter().copied().sum::<f32>() / frame.len() as f32;
            left += extra * 0.5;
            right += extra * 0.5;
        }
        return (left, right);
    };

    let mut left = 0.0;
    let mut right = 0.0;
    let mut add = |position: Position, left_gain: f32, right_gain: f32| {
        if positions.contains(position)
            && let Some(index) = channels.get_canonical_index_for_positioned_channel(position)
            && let Some(sample) = frame.get(index)
        {
            left += sample * left_gain;
            right += sample * right_gain;
        }
    };

    add(Position::FRONT_LEFT, 1.0, 0.0);
    add(Position::FRONT_RIGHT, 0.0, 1.0);
    add(Position::FRONT_CENTER, 0.707, 0.707);
    add(Position::LFE1, 0.25, 0.25);
    add(Position::LFE2, 0.25, 0.25);

    for position in [
        Position::REAR_LEFT,
        Position::FRONT_LEFT_CENTER,
        Position::SIDE_LEFT,
        Position::TOP_FRONT_LEFT,
        Position::TOP_REAR_LEFT,
        Position::TOP_SIDE_LEFT,
        Position::BOTTOM_FRONT_LEFT,
        Position::FRONT_LEFT_WIDE,
    ] {
        add(position, 0.707, 0.0);
    }
    for position in [
        Position::REAR_RIGHT,
        Position::FRONT_RIGHT_CENTER,
        Position::SIDE_RIGHT,
        Position::TOP_FRONT_RIGHT,
        Position::TOP_REAR_RIGHT,
        Position::TOP_SIDE_RIGHT,
        Position::BOTTOM_FRONT_RIGHT,
        Position::FRONT_RIGHT_WIDE,
    ] {
        add(position, 0.0, 0.707);
    }
    for position in [
        Position::REAR_CENTER,
        Position::TOP_CENTER,
        Position::TOP_FRONT_CENTER,
        Position::TOP_REAR_CENTER,
        Position::BOTTOM_FRONT_CENTER,
    ] {
        add(position, 0.5, 0.5);
    }

    (left, right)
}

fn duration_to_frames(duration: std::time::Duration, sample_rate: u32) -> usize {
    let frames = duration.as_secs_f64() * f64::from(sample_rate);
    frames.ceil().max(1.0) as usize
}

#[cfg(test)]
mod tests {
    use symphonia::core::audio::{Channels, Position};

    use super::remix_interleaved;

    #[test]
    fn duplicates_mono_into_stereo_frames() {
        let mut output = Vec::new();
        remix_interleaved(
            &[0.25, -0.5],
            &Channels::from(Position::FRONT_CENTER),
            2,
            &mut output,
        );
        assert_eq!(output, [0.25, 0.25, -0.5, -0.5]);
    }

    #[test]
    fn averages_stereo_into_mono_frames() {
        let mut output = Vec::new();
        remix_interleaved(
            &[1.0, -1.0, 0.5, 0.5],
            &Channels::from(Position::FRONT_LEFT | Position::FRONT_RIGHT),
            1,
            &mut output,
        );
        assert_eq!(output, [0.0, 0.5]);
    }

    #[test]
    fn downmixes_center_without_feeding_lfe_at_full_gain() {
        let channels = Channels::from(
            Position::FRONT_LEFT
                | Position::FRONT_RIGHT
                | Position::FRONT_CENTER
                | Position::LFE1
                | Position::REAR_LEFT
                | Position::REAR_RIGHT,
        );
        let mut output = Vec::new();
        remix_interleaved(&[0.0, 0.0, 1.0, 1.0, 0.0, 0.0], &channels, 2, &mut output);
        assert_eq!(output, [0.957, 0.957]);
    }
}
