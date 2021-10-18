pub mod album;
pub mod error;
pub mod library;
mod manager;
pub mod repo;
pub mod category;
pub mod date;

pub use album::Album;
pub use error::Error;
pub use manager::RepositoryManager;
pub use repo::Repository;

pub type Result<R> = std::result::Result<R, Error>;
