use log::LevelFilter;
use clap::Clap;
use crate::cli::{HandleArgs, AnniArgs};

mod i18n;
mod subcommands;
mod config;
mod args;
mod cli;

#[macro_use]
extern crate anyhow;

#[macro_use]
extern crate log;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // initialize env_logger
    env_logger::builder()
        .filter_level(LevelFilter::Error)
        .filter_module("i18n_embed::requester", LevelFilter::Error)
        .parse_env("ANNI_LOG")
        .format(pretty_env_logger::formatter)
        .init();

    // parse arguments
    let args: AnniArgs = AnniArgs::parse();
    log::debug!("{:#?}", args);
    args.handle()
}
