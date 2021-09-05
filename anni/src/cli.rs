use clap::{Clap, AppSettings};
use crate::ll;
use crate::subcommands::*;
use anni_derive::ClapHandler;

pub trait Handle {
    fn handle(&self) -> anyhow::Result<()>;
}

pub trait HandleArgs<T = ()> {
    fn handle(&self, arg: &T) -> anyhow::Result<()>;
}

#[derive(Clap, ClapHandler, Debug)]
#[clap(name = "Project Anni", version, author)]
#[clap(about = ll ! {"anni-about"})]
#[clap(setting = AppSettings::ColoredHelp)]
pub struct AnniArgs {
    #[clap(subcommand)]
    subcommand: AnniSubcommand,
}

#[derive(Clap, ClapHandler, Debug)]
pub enum AnniSubcommand {
    Flac(FlacSubcommand),
    Split(SplitSubcommand),
    Convention(ConventionSubcommand),
    Repo(RepoSubcommand),
    Get(GetSubcommand),
}
