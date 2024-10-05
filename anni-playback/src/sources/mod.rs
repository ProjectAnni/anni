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

use symphonia_core::io::MediaSource;

pub mod cached_http;
pub mod http;
pub mod streamable;

/// A type that holds an ID and a `std::sync::mpsc::Receiver`.
/// Used for multithreaded download of audio data.
struct Receiver {
    id: u128,
    receiver: std::sync::mpsc::Receiver<(usize, Vec<u8>)>,
}

pub trait AnniSource: MediaSource + IntoBoxedMediaSource {
    /// The duration of underlying source in seconds.
    fn duration_hint(&self) -> Option<u64> {
        None
    }
}

impl MediaSource for Box<dyn AnniSource> {
    fn is_seekable(&self) -> bool {
        self.as_ref().is_seekable()
    }

    fn byte_len(&self) -> Option<u64> {
        self.as_ref().byte_len()
    }
}

impl AnniSource for std::fs::File {}

// helper trait to do upcasting
pub trait IntoBoxedMediaSource {
    fn into_media_source(self: Box<Self>) -> Box<dyn MediaSource>;
}

impl<T: MediaSource + 'static> IntoBoxedMediaSource for T {
    fn into_media_source(self: Box<Self>) -> Box<dyn MediaSource> {
        self
    }
}

impl From<Box<dyn AnniSource>> for Box<dyn MediaSource> {
    fn from(value: Box<dyn AnniSource>) -> Self {
        value.into_media_source()
    }
}

// Specialization is not well-supported so far (even the unstable feature is unstable ww).
// Therefore, we do not provide the default implementation below.
// Users can use a newtype pattern if needed.
//
// default impl<T: MediaSource> AnniSource for T {
//     fn duration_hint(&self) -> Option<u64> {
//         None
//     }
// }
