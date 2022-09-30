pub mod completions;
pub mod convention;
pub mod flac;
pub mod library;
pub mod repo;
pub mod split;
pub mod workspace;

pub use completions::CompletionsSubcommand;
pub use convention::ConventionSubcommand;
pub use flac::FlacSubcommand;
pub use library::LibrarySubcommand;
pub use repo::RepoSubcommand;
pub use split::SplitSubcommand;
pub use workspace::{WorkspaceAction, WorkspaceSubcommand};
