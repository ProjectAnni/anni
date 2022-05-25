pub mod flac;
pub mod split;
pub mod convention;
pub mod repo;
pub mod library;
pub mod completions;

pub use flac::FlacSubcommand;
pub use split::SplitSubcommand;
pub use convention::ConventionSubcommand;
pub use repo::RepoSubcommand;
pub use library::LibrarySubcommand;
pub use completions::CompletionsSubcommand;