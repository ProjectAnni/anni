#[allow(clippy::module_inception)]
mod decoder;
mod opus;

pub use decoder::{Decoder, CODEC_REGISTRY};
