use clap::{App, AppSettings, crate_authors, crate_version};
use crate::subcommands::Subcommands;
use crate::i18n::ClapI18n;
use log::LevelFilter;

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
        .parse_env("ANNI_LOG")
        .format(pretty_env_logger::formatter)
        .init();

    let subcommands: Subcommands = Default::default();
    let matches = App::new("Project Anni")
        .about_ll("anni-about")
        .version(crate_version!())
        .author(crate_authors!())
        .global_setting(AppSettings::ColoredHelp)
        .setting(AppSettings::ArgRequiredElseHelp)
        .subcommands(subcommands.iter())
        .get_matches();

    let (subcommand, matches) = matches.subcommand().unwrap();
    debug!("SubCommand matched: {}", subcommand);
    subcommands.handle(subcommand, matches)
}
