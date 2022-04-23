use log::LevelFilter;
use clap::{Parser, AppSettings};
use anni_clap_handler::Handler;
use crate::subcommands::*;

mod i18n;
mod subcommands;
mod config;
mod args;

#[macro_use]
extern crate anyhow;

#[macro_use]
extern crate log;

#[derive(Parser, Handler, Debug, Clone)]
#[clap(name = "Project Anni", version = env ! ("ANNI_VERSION"), author)]
#[clap(about = ll ! {"anni-about"})]
#[clap(global_setting = AppSettings::DeriveDisplayOrder)]
pub struct AnniArguments {
    #[clap(subcommand)]
    subcommand: AnniSubcommand,
}

#[derive(Parser, Handler, Debug, Clone)]
pub enum AnniSubcommand {
    Flac(FlacSubcommand),
    Split(SplitSubcommand),
    Convention(ConventionSubcommand),
    Repo(RepoSubcommand),
    Library(LibrarySubcommand),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // initialize env_logger
    env_logger::builder()
        .filter_level(LevelFilter::Info)
        .filter_module("i18n_embed::requester", LevelFilter::Error)
        .filter_module("sqlx::query", LevelFilter::Warn)
        .parse_env("ANNI_LOG")
        .format(pretty_env_logger::formatter)
        .init();

    // parse arguments
    let args = AnniArguments::parse();
    log::debug!("{:#?}", args);
    args.run().await
}
