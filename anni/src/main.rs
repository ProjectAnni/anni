#![feature(try_blocks)]
#![allow(incomplete_features)]
#![feature(async_fn_in_trait)]

use crate::subcommands::*;
use clap::Parser;
use clap_handler::Handler;
use log::LevelFilter;

mod args;
mod config;
mod i18n;
mod subcommands;
mod utils;

#[macro_use]
extern crate anyhow;

#[macro_use]
extern crate log;

#[derive(Parser, Handler, Debug, Clone)]
#[clap(name = "Project Anni", version = env!("ANNI_VERSION"), author)]
#[clap(about = ll!("anni-about"))]
#[clap(infer_subcommands = true)]
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
    Completions(CompletionsSubcommand),
    Workspace(WorkspaceSubcommand),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // initialize env_logger
    env_logger::builder()
        .filter_level(LevelFilter::Info)
        .filter_module("i18n_embed::requester", LevelFilter::Off)
        .filter_module("sqlx::query", LevelFilter::Warn)
        .parse_env("ANNI_LOG")
        .format(utils::log::formatter)
        .init();

    // parse arguments
    let args = AnniArguments::parse();
    log::debug!("{:#?}", args);
    args.run().await
}
