// Symphonia
// Copyright (c) 2019-2022 The Project Symphonia Developers.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use rubato::{
    audioadapter_buffers::direct::SequentialSliceOfVecs, Fft, FixedSync,
    Resampler as RubatoResampler,
};
use symphonia::core::audio::{AudioSpec, GenericAudioBufferRef};

pub struct Resampler {
    resampler: Fft<f32>,
    input: Vec<Vec<f32>>,
    scratch: Vec<Vec<f32>>,
    interleaved: Vec<f32>,
    chunk_size: usize,
}

impl Resampler {
    pub fn new(spec: AudioSpec, to_sample_rate: usize, duration: usize) -> Self {
        let num_channels = spec.channels().count();

        let resampler = Fft::<f32>::new(
            spec.rate() as usize,
            to_sample_rate,
            duration,
            num_channels,
            FixedSync::Input,
        )
        .unwrap();
        let chunk_size = resampler.input_frames_next();

        Self {
            resampler,
            input: vec![Vec::with_capacity(chunk_size); num_channels],
            scratch: vec![Vec::new(); num_channels],
            interleaved: Vec::new(),
            chunk_size,
        }
    }

    /// Resamples a planar/non-interleaved input.
    ///
    /// Returns the resampled samples in an interleaved format.
    pub fn resample(&mut self, input: GenericAudioBufferRef<'_>) -> Option<&[f32]> {
        input.copy_to_vecs_planar(&mut self.scratch);
        for (input, scratch) in self.input.iter_mut().zip(&self.scratch) {
            input.extend_from_slice(scratch);
        }

        if self.input.first()?.len() < self.chunk_size {
            return None;
        }

        Some(self.resample_inner())
    }

    /// Resample any remaining samples in the resample buffer.
    pub fn flush(&mut self) -> Option<&[f32]> {
        let len = self.input.first()?.len();

        if len == 0 {
            return None;
        }

        // Rubato's synchronous FFT resampler consumes a fixed number of frames. Preserve the
        // previous implementation's flushing semantics by padding the final chunk with silence.
        for channel in &mut self.input {
            channel.resize(self.chunk_size, 0.0);
        }

        Some(self.resample_inner())
    }

    fn resample_inner(&mut self) -> &[f32] {
        let num_channels = self.input.len();
        let input = SequentialSliceOfVecs::new(&self.input, num_channels, self.chunk_size).unwrap();
        let output = self.resampler.process(&input, None).unwrap();

        self.interleaved = output.take_data();

        for channel in &mut self.input {
            channel.drain(..self.chunk_size);
        }

        &self.interleaved
    }
}

#[cfg(test)]
mod tests {
    use symphonia::core::audio::{
        AsGenericAudioBufferRef, AudioBuffer, AudioSpec, Channels, Position,
    };

    use super::Resampler;

    #[test]
    fn resamples_a_complete_stereo_chunk() {
        let spec = AudioSpec::new(
            44_100,
            Channels::from(Position::FRONT_LEFT | Position::FRONT_RIGHT),
        );
        let mut input = AudioBuffer::<f32>::new(spec.clone(), 441);
        input.resize_with_silence(441);

        let mut resampler = Resampler::new(spec, 48_000, 441);
        let output = resampler
            .resample(input.as_generic_audio_buffer_ref())
            .expect("a complete input chunk should be resampled");

        assert!(!output.is_empty());
        assert_eq!(output.len() % 2, 0);
    }
}
