#[macro_use]
extern crate nom;
#[macro_use]
extern crate num_derive;

pub mod parser;
mod stream;
mod header;
mod utils;

pub use stream::Stream;
pub use header::{MetadataBlock, MetadataBlockData};

pub mod blocks;
pub mod prelude;
pub mod error;