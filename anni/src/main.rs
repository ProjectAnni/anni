use clap::{App, AppSettings, crate_authors, crate_version};
use log::LevelFilter;
use crate::subcommands::Subcommands;
use crate::i18n::ClapI18n;

mod i18n;
mod subcommands;
mod config;

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
        .filter_module("anni::subcommands::convention", LevelFilter::Info)
        .parse_env("ANNI_LOG")
        .init();

    let subcommands: Subcommands = Default::default();
    let matches = App::new("Project Anni")
        .about_ll("anni-about")
        .version(crate_version!())
        .author(crate_authors!())
        .global_setting(AppSettings::ArgRequiredElseHelp)
        .global_setting(AppSettings::ColoredHelp)
        .subcommands(subcommands.iter())
        .get_matches();

    let (subcommand, matches) = matches.subcommand().unwrap();
    debug!("SubCommand matched: {}", subcommand);
    subcommands.handle(subcommand, matches)
}
