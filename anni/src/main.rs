use clap::{App, AppSettings, crate_authors, crate_version, Arg};
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
    let subcommands: Subcommands = Default::default();
    let matches = App::new("Project Anni")
        .about_ll("anni-about")
        .version(crate_version!())
        .author(crate_authors!())
        .global_setting(AppSettings::ColoredHelp)
        .setting(AppSettings::ArgRequiredElseHelp)
        .subcommands(subcommands.iter())
        .arg(Arg::new("log")
            .takes_value(true)
        )
        .get_matches();

    let log_config = matches.value_of("log")
        .map(|p| std::fs::read_to_string(p).unwrap())
        .unwrap_or(r#"
refresh_rate = "30 seconds"

[appenders.stderr]
kind = "console"
target = "stderr"
encoder = { pattern = "[{h({l})}][{t}] {m}{n}" }

[root]
level = "warn"
appenders = ["stderr"]

[loggers]
"i18n_embed::requester" = { level = "error" }
"anni" = { level = "info" }
"#.to_string());
    let log_config = toml::from_str(&log_config)?;
    log4rs::init_raw_config(log_config).unwrap();

    let (subcommand, matches) = matches.subcommand().unwrap();
    debug!("SubCommand matched: {}", subcommand);
    subcommands.handle(subcommand, matches)
}
