pub mod flac;
pub mod split;
pub mod convention;
pub mod repo;

pub use flac::FlacSubcommand;
pub use split::SplitSubcommand;
pub use convention::ConventionSubcommand;
pub use repo::RepoSubcommand;