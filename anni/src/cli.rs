use clap::{Clap, AppSettings};
use crate::ll;
use crate::subcommands::*;
use anni_derive::ClapHandler;

pub trait Handle {
    #[inline(always)]
    fn handle(&self) -> anyhow::Result<()> {
        self.handle_subcommand()
    }

    #[inline(always)]
    fn handle_subcommand(&self) -> anyhow::Result<()> {
        Ok(())
    }
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
