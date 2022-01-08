pub mod error;
pub mod library;
mod manager;
pub mod models;

pub mod prelude {
    pub use crate::models::*;
    pub use crate::error::Error;

    pub type RepoResult<R> = std::result::Result<R, Error>;
}

#[cfg(feature = "db")]
pub mod db;

pub use manager::RepositoryManager;
