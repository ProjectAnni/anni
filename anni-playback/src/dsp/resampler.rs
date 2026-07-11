// Symphonia
// Copyright (c) 2019-2022 The Project Symphonia Developers.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use anyhow::Context;
use rubato::{
    audioadapter_buffers::direct::SequentialSliceOfVecs, Fft, FixedSync, Indexing,
    Resampler as RubatoResampler,
};
use symphonia::core::audio::{AudioSpec, GenericAudioBufferRef};

fn copy_to_planar_scratch(input: GenericAudioBufferRef<'_>, scratch: &mut Vec<Vec<f32>>) {
    for channel in scratch.iter_mut() {
        channel.clear();
    }
    input.copy_to_vecs_planar(scratch);
}

pub struct Resampler {
    resampler: Fft<f32>,
    input: Vec<Vec<f32>>,
    scratch: Vec<Vec<f32>>,
    interleaved: Vec<f32>,
    chunk_size: usize,
    channels: usize,
    input_rate: usize,
    output_rate: usize,
    delay_remaining: usize,
    total_input_frames: usize,
    emitted_frames: usize,
    finished: bool,
}

impl Resampler {
    pub fn new(spec: AudioSpec, to_sample_rate: usize, duration: usize) -> anyhow::Result<Self> {
        let channels = spec.channels().count();
        let input_rate = spec.rate() as usize;
        let resampler = Fft::<f32>::new(
            input_rate,
            to_sample_rate,
            duration.max(1),
            channels,
            FixedSync::Input,
        )
        .context("failed to construct FFT resampler")?;
        let chunk_size = resampler.input_frames_next();
        let delay_remaining = resampler.output_delay();

        Ok(Self {
            resampler,
            input: vec![Vec::with_capacity(chunk_size); channels],
            scratch: vec![Vec::new(); channels],
            interleaved: Vec::new(),
            chunk_size,
            channels,
            input_rate,
            output_rate: to_sample_rate,
            delay_remaining,
            total_input_frames: 0,
            emitted_frames: 0,
            finished: false,
        })
    }

    /// Resamples every complete chunk currently available. Startup delay is
    /// removed, so the returned samples are ready to append to the output ring.
    pub fn resample(&mut self, input: GenericAudioBufferRef<'_>) -> Option<&[f32]> {
        if self.finished {
            return None;
        }

        self.total_input_frames += input.frames();
        copy_to_planar_scratch(input, &mut self.scratch);
        for (buffer, scratch) in self.input.iter_mut().zip(&self.scratch) {
            buffer.extend_from_slice(scratch);
        }

        let mut output = std::mem::take(&mut self.interleaved);
        output.clear();
        let target_received = self.target_frames();
        while self
            .input
            .first()
            .is_some_and(|channel| channel.len() >= self.chunk_size)
        {
            let chunk = self.process_chunk(None);
            self.append_valid_output(chunk, Some(target_received), &mut output);
            for channel in &mut self.input {
                channel.drain(..self.chunk_size);
            }
        }

        self.interleaved = output;
        (!self.interleaved.is_empty()).then_some(&self.interleaved)
    }

    /// Flushes filter delay and emits exactly ceil(input_frames * ratio)
    /// frames. Padding supplied to Rubato is never exposed to the caller.
    pub fn flush(&mut self) -> Option<&[f32]> {
        if self.finished {
            return None;
        }
        self.finished = true;

        let target_frames = self.target_frames();
        let mut output = std::mem::take(&mut self.interleaved);
        output.clear();
        let remaining = self.input.first().map_or(0, Vec::len);

        if remaining > 0 {
            for channel in &mut self.input {
                channel.resize(self.chunk_size, 0.0);
            }
            let indexing = Indexing {
                partial_len: Some(remaining),
                ..Default::default()
            };
            let chunk = self.process_chunk(Some(&indexing));
            self.append_valid_output(chunk, Some(target_frames), &mut output);
            for channel in &mut self.input {
                channel.clear();
            }
        }

        while self.emitted_frames < target_frames {
            let emitted_before = self.emitted_frames;
            let delay_before = self.delay_remaining;
            for channel in &mut self.input {
                channel.resize(self.chunk_size, 0.0);
            }
            let indexing = Indexing {
                partial_len: Some(0),
                ..Default::default()
            };
            let chunk = self.process_chunk(Some(&indexing));
            self.append_valid_output(chunk, Some(target_frames), &mut output);
            for channel in &mut self.input {
                channel.clear();
            }
            if self.emitted_frames == emitted_before && self.delay_remaining == delay_before {
                log::error!("resampler flush made no progress before reaching its target");
                break;
            }
        }

        self.interleaved = output;
        (!self.interleaved.is_empty()).then_some(&self.interleaved)
    }

    fn process_chunk(&mut self, indexing: Option<&Indexing>) -> Vec<f32> {
        let input =
            SequentialSliceOfVecs::new(&self.input, self.channels, self.chunk_size).unwrap();
        self.resampler
            .process(&input, indexing)
            .unwrap()
            .take_data()
    }

    fn target_frames(&self) -> usize {
        ((self.total_input_frames as u128 * self.output_rate as u128)
            .div_ceil(self.input_rate as u128)) as usize
    }

    fn append_valid_output(
        &mut self,
        chunk: Vec<f32>,
        target_frames: Option<usize>,
        output: &mut Vec<f32>,
    ) {
        let frames = chunk.len() / self.channels;
        let skip = self.delay_remaining.min(frames);
        self.delay_remaining -= skip;

        let available = frames - skip;
        let allowed = target_frames
            .map(|target| target.saturating_sub(self.emitted_frames))
            .unwrap_or(available)
            .min(available);
        let start = skip * self.channels;
        let end = start + allowed * self.channels;
        output.extend_from_slice(&chunk[start..end]);
        self.emitted_frames += allowed;
    }
}

#[cfg(test)]
mod tests {
    use symphonia::core::audio::{
        AsGenericAudioBufferRef, AudioBuffer, AudioSpec, Channels, Position,
    };

    use super::{copy_to_planar_scratch, Resampler};

    fn stereo_spec(rate: u32) -> AudioSpec {
        AudioSpec::new(
            rate,
            Channels::from(Position::FRONT_LEFT | Position::FRONT_RIGHT),
        )
    }

    #[test]
    fn replaces_planar_scratch_samples_instead_of_appending() {
        let spec = stereo_spec(44_100);
        let mut first = AudioBuffer::<f32>::new(spec.clone(), 2);
        first.resize_with_silence(2);
        let mut second = AudioBuffer::<f32>::new(spec, 1);
        second.resize_with_silence(1);
        let mut scratch = vec![vec![1.0; 8], vec![1.0; 8]];

        copy_to_planar_scratch(first.as_generic_audio_buffer_ref(), &mut scratch);
        assert!(scratch.iter().all(|channel| channel.len() == 2));
        copy_to_planar_scratch(second.as_generic_audio_buffer_ref(), &mut scratch);
        assert!(scratch.iter().all(|channel| channel.len() == 1));
    }

    #[test]
    fn reuses_the_interleaved_output_allocation() {
        let spec = stereo_spec(44_100);
        let mut input = AudioBuffer::<f32>::new(spec.clone(), 1);
        input.resize_with_silence(1);
        let mut resampler = Resampler::new(spec, 48_000, 441).unwrap();
        resampler.interleaved.reserve(4_096);
        let capacity = resampler.interleaved.capacity();

        assert!(resampler
            .resample(input.as_generic_audio_buffer_ref())
            .is_none());
        assert_eq!(resampler.interleaved.capacity(), capacity);
    }

    #[test]
    fn flush_emits_exact_resampled_duration_without_padding() {
        let spec = stereo_spec(44_100);
        let input_frames = 1_001;
        let mut input = AudioBuffer::<f32>::new(spec.clone(), input_frames);
        input.resize_with_silence(input_frames);
        let mut resampler = Resampler::new(spec, 48_000, 441).unwrap();

        let streamed = resampler
            .resample(input.as_generic_audio_buffer_ref())
            .map_or(0, <[f32]>::len);
        let flushed = resampler.flush().map_or(0, <[f32]>::len);
        let expected_frames = (input_frames as f64 * 48_000.0 / 44_100.0).ceil() as usize;

        assert_eq!(streamed + flushed, expected_frames * 2);
    }

    #[test]
    fn flushes_delay_when_input_ends_on_a_chunk_boundary() {
        let spec = stereo_spec(44_100);
        let mut resampler = Resampler::new(spec.clone(), 48_000, 441).unwrap();
        let frames = resampler.chunk_size;
        let mut input = AudioBuffer::<f32>::new(spec, frames);
        input.resize_with_silence(frames);

        let streamed = resampler
            .resample(input.as_generic_audio_buffer_ref())
            .map_or(0, <[f32]>::len);
        let flushed = resampler.flush().map_or(0, <[f32]>::len);
        let expected_frames = (frames as f64 * 48_000.0 / 44_100.0).ceil() as usize;

        assert_eq!(streamed + flushed, expected_frames * 2);
    }
}
