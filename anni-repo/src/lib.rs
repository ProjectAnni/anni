pub mod error;
pub mod library;
mod manager;
pub mod repo;
pub mod db;
pub mod models;

pub mod prelude {
    pub use crate::models::*;
    pub use crate::repo::Repository;
    pub use crate::error::Error;

    pub type RepoResult<R> = std::result::Result<R, Error>;
}

pub use manager::RepositoryManager;
