mod rows;

pub const DB_VERSION: &str = "1.1";

#[cfg(feature = "db-read")]
mod read;

#[cfg(feature = "db-read")]
pub use read::RepoDatabaseRead;

#[cfg(feature = "db-write")]
mod write;

#[cfg(feature = "db-write")]
pub use write::RepoDatabaseWrite;

#[cfg(target_arch = "wasm32")]
pub(crate) mod fs;
