pub mod flac;
pub mod split;
pub mod convention;
pub mod repo;
pub mod get;

pub trait Subcommand {
    fn name(&self) -> &'static str;
    fn create(&self) -> clap::App<'static>;
    fn handle(&self, matches: &clap::ArgMatches) -> anyhow::Result<()>;
}
