pub mod error;
pub mod library;
mod manager;
pub mod models;

pub mod prelude {
    pub use crate::models::*;
    pub use crate::error::Error;

    pub type RepoResult<R> = Result<R, Error>;
}

pub mod db;
pub(crate) mod utils;

pub use manager::{RepositoryManager, OwnedRepositoryManager};

#[cfg(feature = "git")]
pub use utils::git::setup_git2;
