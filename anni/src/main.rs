use std::env;
use std::path::PathBuf;

use clap::{App, AppSettings, crate_authors, crate_version};
use log::LevelFilter;
use crate::subcommands::Subcommands;

mod encoding;
mod i18n;
mod subcommands;

#[macro_use]
extern crate anyhow;

#[macro_use]
extern crate log;

fn main() -> anyhow::Result<()> {
    let config = env::var("ANNI_CONFIG")
        .map(|cfg| PathBuf::from(cfg))
        .unwrap_or({
            let dir = directories_next::ProjectDirs::from("moe", "mmf", "anni").expect("Failed to get project dirs.");
            dir.config_dir().join("config.conf")
        });
    if config.exists() {
        // apply env from config path
        dotenv::from_path(&config)?;
        // initialize env_logger
        env_logger::builder()
            .filter_level(LevelFilter::Warn)
            .parse_env("ANNI_LOG")
            .init();
        info!("Read config from: {:?}", config);
    } else {
        // initialize env_logger
        env_logger::builder()
            .filter_level(LevelFilter::Warn)
            .parse_env("ANNI_LOG")
            .init();
        // config file not exist
        warn!("Config file not exist: {:?}", config);
    }

    let subcommands: Subcommands = Default::default();
    let matches = App::new("Project Anni")
        .about(fl!("anni-about"))
        .version(crate_version!())
        .author(crate_authors!())
        .setting(AppSettings::ArgRequiredElseHelp)
        .setting(AppSettings::ColoredHelp)
        .subcommands(subcommands.iter())
        .get_matches();

    let (subcommand, matches) = matches.subcommand().unwrap();
    debug!("SubCommand matched: {}", subcommand);
    subcommands.handle(subcommand, matches)
}
