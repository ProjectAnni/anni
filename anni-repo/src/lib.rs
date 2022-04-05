pub mod error;
pub mod library;
mod manager;
pub mod models;

pub mod prelude {
    pub use crate::models::*;
    pub use crate::error::Error;

    pub type RepoResult<R> = std::result::Result<R, Error>;
}

pub mod db;
pub(crate) mod utils;

pub use manager::{RepositoryManager, OwnedRepositoryManager};
