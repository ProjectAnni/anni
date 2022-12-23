pub mod error;
pub mod library;
mod manager;
pub mod models;

#[cfg(feature = "search")]
pub mod search;

pub mod prelude {
    pub use crate::error::Error;
    pub use crate::models::*;

    pub type RepoResult<R> = Result<R, Error>;
}

pub mod db;
pub(crate) mod utils;

pub use manager::{OwnedRepositoryManager, RepositoryManager};

#[cfg(feature = "git")]
pub use utils::git::setup_git2;
