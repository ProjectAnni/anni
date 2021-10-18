use log::LevelFilter;
use clap::Parser;
use anni_derive::ClapHandler;
use anni_common::traits::Handle;
use crate::subcommands::*;

mod i18n;
mod subcommands;
mod config;
mod args;

#[macro_use]
extern crate anyhow;

#[macro_use]
extern crate log;

#[derive(Parser, ClapHandler, Debug)]
#[clap(name = "Project Anni", version, author)]
#[clap(about = ll ! {"anni-about"})]
pub struct AnniArguments {
    #[clap(subcommand)]
    subcommand: AnniSubcommand,
}

#[derive(Parser, ClapHandler, Debug)]
pub enum AnniSubcommand {
    Flac(FlacSubcommand),
    Split(SplitSubcommand),
    Convention(ConventionSubcommand),
    Repo(RepoSubcommand),
    Get(GetSubcommand),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // initialize env_logger
    env_logger::builder()
        .filter_level(LevelFilter::Info)
        .filter_module("i18n_embed::requester", LevelFilter::Error)
        .parse_env("ANNI_LOG")
        .format(pretty_env_logger::formatter)
        .init();

    // parse arguments
    let args = AnniArguments::parse();
    log::debug!("{:#?}", args);
    args.handle()
}
