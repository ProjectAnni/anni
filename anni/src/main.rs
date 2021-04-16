use clap::{App, Arg, ArgGroup, crate_authors, crate_version, AppSettings};
use log::LevelFilter;
use std::env;
use std::path::PathBuf;

mod flac;
mod encoding;
mod cue;
mod i18n;
mod repo;
mod split;
mod convention;

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

    let matches = App::new("Project Anni")
        .about(fl!("anni-about"))
        .version(crate_version!())
        .author(crate_authors!())
        .setting(AppSettings::ArgRequiredElseHelp)
        .setting(AppSettings::ColoredHelp)
        .subcommand(App::new("flac")
            .about(fl!("flac"))
            .arg(Arg::new("flac.export")
                .about(fl!("flac-export"))
                .long("export")
                .short('e')
            )
            .arg(Arg::new("flac.export.type")
                .about(fl!("flac-export-type"))
                .long("export-type")
                .short('t')
                .takes_value(true)
                .default_value("tag")
                .possible_values(&[
                    // block types
                    "info", "application", "seektable", "cue",
                    // comment & alias
                    "comment", "tag",
                    // common picture
                    "picture",
                    // picture: cover
                    "cover",
                    // list
                    "list", "all",
                ])
            )
            .arg(Arg::new("flac.export.to")
                .about(fl!("flac-export-to"))
                .long("export-to")
                .short('o')
                .takes_value(true)
                .default_value("-")
            )
            .group(ArgGroup::new("group.flac.export")
                .args(&["flac.export", "flac.export.type", "flac.export.to"])
                .multiple(true)
            )
            .arg(Arg::new("Filename")
                .takes_value(true)
                .min_values(1)
            )
        )
        .subcommand(App::new("cue")
            .about(fl!("cue"))
            .arg(Arg::new("cue.file")
                .about(fl!("cue-file"))
                .long("file")
                .short('f')
                .takes_value(true)
            )
            .arg(Arg::new("cue.dir")
                .about(fl!("cue-dir"))
                .long("dir")
                .short('d')
            )
            .group(ArgGroup::new("cue.source")
                .args(&["cue.file", "cue.dir"])
                .required(true)
            )
            .arg(Arg::new("cue.tagsh")
                .about(fl!("cue-tagsh"))
                .long("tag-sh")
                .short('t')
            )
            .arg(Arg::new("Filename")
                .takes_value(true)
                .min_values(1)
            )
        )
        .subcommand(App::new("split")
            .about(fl!("split"))
            .arg(Arg::new("split.format.input")
                .about(fl!("split-format-input"))
                .long("input-format")
                .short('i')
                .takes_value(true)
                .default_value("wav")
                .possible_values(&["wav", "flac", "ape"])
                .env("Anni_Split_Input_Format")
            )
            .arg(Arg::new("split.format.output")
                .about(fl!("split-format-output"))
                .long("output-format")
                .short('o')
                .takes_value(true)
                .default_value("flac")
                .possible_values(&["wav", "flac"])
                .env("Anni_Split_Output_Format")
            )
            .arg(Arg::new("Filename")
                .required(true)
                .takes_value(true)
            )
        )
        .subcommand(App::new("convention")
            .about(fl!("convention"))
            .alias("conv")
            .subcommand(App::new("check")
                .about(fl!("convention-check"))
            )
        )
        .subcommand(App::new("repo")
            .about(fl!("repo"))
            .arg(Arg::new("repo.root")
                .about(fl!("repo-root"))
                .long("root")
                .env("ANNI_ROOT")
                .takes_value(true)
                .required(true)
            )
            .subcommand(App::new("add")
                .about(fl!("repo-add"))
                .arg(Arg::new("edit")
                    .about(fl!("repo-add-edit"))
                    .long("edit")
                    .short('e')
                )
                .arg(Arg::new("Filename")
                    .takes_value(true)
                    .min_values(1)
                )
            )
            .subcommand(App::new("edit")
                .about(fl!("repo-edit"))
                .arg(Arg::new("Filename")
                    .takes_value(true)
                    .min_values(1)
                )
            )
            .subcommand(App::new("apply")
                .about(fl!("repo-apply"))
                .arg(Arg::new("Filename")
                    .takes_value(true)
                    .min_values(1)
                )
            )
        )
        .get_matches();

    if let Some(matches) = matches.subcommand_matches("flac") {
        flac::handle_flac(matches)?;
    } else if let Some(matches) = matches.subcommand_matches("cue") {
        debug!("SubCommand matched: cue");
        cue::handle_cue(matches)?;
    } else if let Some(matches) = matches.subcommand_matches("split") {
        debug!("SubCommand matched: split");
        split::handle_split(matches)?;
    } else if let Some(matches) = matches.subcommand_matches("convention") {
        debug!("SubCommand matched: convention");
        convention::handle_convention(matches)?;
    } else if let Some(matches) = matches.subcommand_matches("repo") {
        debug!("SubCommand matched: repo");
        repo::handle_repo(matches)?;
    }

    Ok(())
}
