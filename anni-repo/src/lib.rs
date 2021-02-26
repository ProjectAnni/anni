pub mod album;
pub mod repo;
pub mod structure;
mod manager;
mod error;

pub use repo::Repository;
pub use album::Album;
pub use manager::RepositoryManager;
pub use error::Error;

pub use toml::value::Datetime;

pub type Result<R> = std::result::Result<R, Error>;