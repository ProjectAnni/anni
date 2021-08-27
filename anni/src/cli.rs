use clap::{Clap, AppSettings};
use crate::ll;
use crate::subcommands::*;

pub trait Handle {
    fn handle(&self) -> anyhow::Result<()>;
}

pub trait HandleArgs<T = ()> {
    fn handle(&self, arg: &T) -> anyhow::Result<()>;
}

#[derive(Clap, Debug)]
#[clap(name = "Project Anni", version, author)]
#[clap(about = ll ! {"anni-about"})]
#[clap(setting = AppSettings::ColoredHelp)]
pub struct AnniArgs {
    #[clap(subcommand)]
    subcommand: AnniSubcommand,
}

#[derive(Clap, Debug)]
pub enum AnniSubcommand {
    Flac(FlacSubcommand),
    Split(SplitSubcommand),
    Convention(ConventionSubcommand),
    Repo(RepoSubcommand),
    Get(GetSubcommand),
}

impl Handle for AnniArgs {
    fn handle(&self) -> anyhow::Result<()> {
        self.subcommand.handle()
    }
}

impl Handle for AnniSubcommand {
    fn handle(&self) -> anyhow::Result<()> {
        match self {
            AnniSubcommand::Flac(flac) => flac.handle(),
            AnniSubcommand::Split(split) => split.handle(),
            AnniSubcommand::Convention(conv) => conv.handle(),
            AnniSubcommand::Repo(repo) => repo.handle(),
            AnniSubcommand::Get(get) => get.handle(),
        }
    }
}
