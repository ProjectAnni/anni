// Modified from https://github.com/serenity-rs/songbird/blob/next/src/input/codecs/opus.rs
//
// ISC License (ISC)
//
// Copyright (c) 2020, Songbird Contributors
//
// Permission to use, copy, modify, and/or distribute this software for any purpose
// with or without fee is hereby granted, provided that the above copyright notice
// and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES WITH
// REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF MERCHANTABILITY AND
// FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY SPECIAL, DIRECT,
// INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS
// OF USE, DATA OR PROFITS, WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER
// TORTIOUS ACTION, ARISING OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF
// THIS SOFTWARE.

use audiopus::{
    coder::{Decoder as AudiopusDecoder, GenericCtl},
    Channels as OpusChannels, Error as OpusError, ErrorCode, SampleRate,
};
use symphonia_core::{
    audio::{
        AsGenericAudioBufferRef, AudioBuffer, AudioMut, AudioSpec, Channels, GenericAudioBufferRef,
        Position,
    },
    codecs::{
        audio::{
            well_known::CODEC_ID_OPUS, AudioCodecParameters, AudioDecoder, AudioDecoderOptions,
            FinalizeResult,
        },
        registry::{RegisterableAudioDecoder, SupportedAudioCodec},
        CodecInfo,
    },
    errors::{decode_error, Result as SymphResult},
    packet::PacketRef,
};

const SAMPLE_RATE: SampleRate = SampleRate::Hz48000;

const SAMPLE_RATE_RAW: usize = 48_000;

/// This is equally the number of stereo (joint) samples in an audio frame.
const MONO_FRAME_SIZE: usize = SAMPLE_RATE_RAW / 1000 * 60; // 60ms is the max frame size

/// Number of individual samples in one complete frame of stereo audio.
const STEREO_FRAME_SIZE: usize = 2 * MONO_FRAME_SIZE;

fn stereo_spec() -> AudioSpec {
    AudioSpec::new(
        SAMPLE_RATE_RAW as u32,
        Channels::from(Position::FRONT_LEFT | Position::FRONT_RIGHT),
    )
}

/// Opus decoder for symphonia, based on libopus v1.3 (via [`audiopus`]).
pub struct OpusDecoder {
    inner: AudiopusDecoder,
    params: AudioCodecParameters,
    buf: AudioBuffer<f32>,
    rawbuf: Vec<f32>,
}

/// # SAFETY
/// The underlying Opus decoder (currently) requires only a `&self` parameter
/// to decode given packets, which is likely a mistaken decision.
///
/// This struct makes stronger assumptions and only touches FFI decoder state with a
/// `&mut self`, preventing data races via `&OpusDecoder` as required by `impl Sync`.
/// No access to other internal state relies on unsafety or crosses FFI.
unsafe impl Sync for OpusDecoder {}

impl OpusDecoder {
    fn try_new(params: &AudioCodecParameters, _options: &AudioDecoderOptions) -> SymphResult<Self> {
        let inner = AudiopusDecoder::new(SAMPLE_RATE, OpusChannels::Stereo).unwrap();

        let mut params = params.clone();
        params
            .with_sample_rate(SAMPLE_RATE_RAW as u32)
            .with_channels(stereo_spec().channels().clone());

        Ok(Self {
            inner,
            params,
            buf: AudioBuffer::new(stereo_spec(), MONO_FRAME_SIZE),
            rawbuf: vec![0.0f32; STEREO_FRAME_SIZE],
        })
    }

    fn decode_inner(&mut self, packet: &PacketRef<'_>) -> SymphResult<()> {
        let sample_count = loop {
            let packet = if packet.data.is_empty() {
                None
            } else if let Ok(packet) = packet.data.try_into() {
                Some(packet)
            } else {
                return decode_error("Opus packet was too large (greater than i32::MAX bytes).");
            };
            let output = (&mut self.rawbuf[..]).try_into().expect(
                "the Opus decode buffer is kept below i32::MAX elements by the growth logic",
            );

            match self.inner.decode_float(packet, output, false) {
                Ok(value) => break value,
                Err(OpusError::Opus(ErrorCode::BufferTooSmall)) => {
                    let new_size = (self.rawbuf.len() * 2).min(i32::MAX as usize);
                    if new_size == self.rawbuf.len() {
                        return decode_error(
                            "Opus frame too big: cannot expand decode buffer any further.",
                        );
                    }

                    self.rawbuf.resize(new_size, 0.0);
                    self.buf = AudioBuffer::new(stereo_spec(), self.rawbuf.len() / 2);
                }
                Err(_) => return decode_error("Opus decode error"),
            }
        };

        self.buf.clear();
        self.buf.resize_uninit(sample_count);

        // Opus is currently decoded as stereo, matching the previous implementation.
        for channel in 0..2 {
            let source = self.rawbuf.chunks_exact(2).map(|frame| frame[channel]);
            for (target, sample) in self.buf.plane_mut(channel).unwrap().iter_mut().zip(source) {
                *target = sample;
            }
        }

        Ok(())
    }
}

impl AudioDecoder for OpusDecoder {
    fn reset(&mut self) {
        _ = self.inner.reset_state();
    }

    fn codec_info(&self) -> &CodecInfo {
        &Self::supported_codecs()[0].info
    }

    fn codec_params(&self) -> &AudioCodecParameters {
        &self.params
    }

    fn decode_ref(&mut self, packet: &PacketRef<'_>) -> SymphResult<GenericAudioBufferRef<'_>> {
        if let Err(error) = self.decode_inner(packet) {
            self.buf.clear();
            Err(error)
        } else {
            Ok(self.buf.as_generic_audio_buffer_ref())
        }
    }

    fn finalize(&mut self) -> FinalizeResult {
        FinalizeResult::default()
    }

    fn last_decoded(&self) -> GenericAudioBufferRef<'_> {
        self.buf.as_generic_audio_buffer_ref()
    }
}

impl RegisterableAudioDecoder for OpusDecoder {
    fn try_registry_new(
        params: &AudioCodecParameters,
        options: &AudioDecoderOptions,
    ) -> SymphResult<Box<dyn AudioDecoder>> {
        Ok(Box::new(Self::try_new(params, options)?))
    }

    fn supported_codecs() -> &'static [SupportedAudioCodec] {
        &[symphonia_core::support_audio_codec!(
            CODEC_ID_OPUS,
            "opus",
            "libopus (1.5+, audiopus)"
        )]
    }
}
