use clap::{Clap, AppSettings};
use crate::ll;

pub trait HandleArgs {
    fn handle(&self) -> anyhow::Result<()>;
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
    Flac(crate::subcommands::flac::FlacSubcommand),
    Split(crate::subcommands::split::SplitSubcommand),
}

impl HandleArgs for AnniArgs {
    fn handle(&self) -> anyhow::Result<()> {
        self.subcommand.handle()
    }
}

impl HandleArgs for AnniSubcommand {
    fn handle(&self) -> anyhow::Result<()> {
        match self {
            AnniSubcommand::Flac(flac) => flac.handle(),
            AnniSubcommand::Split(split) => split.handle(),
        }
    }
}
