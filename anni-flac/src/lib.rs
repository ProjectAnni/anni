#[macro_use]
extern crate nom;
#[macro_use]
extern crate num_derive;

mod parser;
mod stream;
pub mod decoder;
mod header;
mod common;

pub use parser::*;
pub use stream::Stream;
use crate::decoder::FlacError;

pub type Result<I> = std::result::Result<I, FlacError>;

pub use common::{Decode, DecodeSized};

pub mod blocks;