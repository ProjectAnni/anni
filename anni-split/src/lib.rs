#![feature(impl_trait_in_assoc_type)]

pub mod codec;
mod error;

pub use codec::{Decoder, Encoder};
pub use error::SplitError;
