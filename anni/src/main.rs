use clap::{App, Arg, ArgGroup, crate_authors, crate_version, SubCommand, AppSettings};
use log::LevelFilter;

mod flac;
mod encoding;
mod cue;
mod i18n;
mod repo;
mod split;

#[macro_use]
extern crate anyhow;

#[macro_use]
extern crate log;

fn main() -> anyhow::Result<()> {
    env_logger::builder()
        .filter_level(LevelFilter::Warn)
        .parse_env("ANNI_LOG")
        .init();

    let matches = App::new("Project Anni")
        .about(fl!("anni-about"))
        .version(crate_version!())
        .author(crate_authors!())
        .setting(AppSettings::ArgRequiredElseHelp)
        .subcommand(SubCommand::with_name("flac")
            .about(fl!("flac"))
            .arg(Arg::with_name("flac.check")
                .help(fl!("flac-check"))
                .long("check")
                .short("c")
            )
            .arg(Arg::with_name("flac.report.format")
                .help(fl!("flac-report-format"))
                .long("report-format")
                .short("f")
                .required(true)
                .takes_value(true)
                .default_value("table")
                .possible_values(&["table", "markdown"])
            )
            .arg(Arg::with_name("flac.export")
                .help(fl!("flac-export"))
                .long("export")
                .short("e")
            )
            .arg(Arg::with_name("flac.export.type")
                .help(fl!("flac-export-type"))
                .long("export-type")
                .short("t")
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
            .arg(Arg::with_name("flac.export.to")
                .help(fl!("flac-export-to"))
                .long("export-to")
                .short("o")
                .takes_value(true)
                .default_value("-")
            )
            .group(ArgGroup::with_name("group.flac")
                .args(&["flac.check", "flac.export"])
                .required(true)
            )
            .group(ArgGroup::with_name("group.flac.export")
                .args(&["flac.export", "flac.export.type", "flac.export.to"])
                .multiple(true)
            )
            .arg(Arg::with_name("Filename")
                .takes_value(true)
                .empty_values(false)
                .multiple(true)
            )
        )
        .subcommand(SubCommand::with_name("cue")
            .about(fl!("cue"))
            .arg(Arg::with_name("cue.file")
                .help(fl!("cue-file"))
                .long("file")
                .short("f")
                .takes_value(true)
            )
            .arg(Arg::with_name("cue.dir")
                .help(fl!("cue-dir"))
                .long("dir")
                .short("d")
            )
            .group(ArgGroup::with_name("cue.source")
                .args(&["cue.file", "cue.dir"])
                .required(true)
            )
            .arg(Arg::with_name("cue.tagsh")
                .help(fl!("cue-tagsh"))
                .long("tag-sh")
                .short("t")
            )
            .arg(Arg::with_name("Filename")
                .takes_value(true)
                .empty_values(false)
                .multiple(true)
            )
        )
        .subcommand(SubCommand::with_name("split")
            .about(fl!("split"))
            .arg(Arg::with_name("split.format.input")
                .help(fl!("split-format-input"))
                .long("input-format")
                .short("i")
                .takes_value(true)
                .default_value("wav")
                .possible_values(&["wav", "flac", "ape"])
            )
            .arg(Arg::with_name("split.format.output")
                .help(fl!("split-format-output"))
                .long("output-format")
                .short("o")
                .takes_value(true)
                .default_value("flac")
                .possible_values(&["wav", "flac"])
            )
            .arg(Arg::with_name("Filename")
                .takes_value(true)
                .empty_values(false)
            )
        )
        .subcommand(SubCommand::with_name("repo")
            .about(fl!("repo"))
            .arg(Arg::with_name("repo.root")
                .help(fl!("repo-root"))
                .long("root")
                .env("ANNI_ROOT")
                .takes_value(true)
                .required(true)
            )
            .subcommand(SubCommand::with_name("add")
                .about(fl!("repo-add"))
                .arg(Arg::with_name("edit")
                    .help(fl!("repo-add-edit"))
                    .long("edit")
                    .short("e")
                )
                .arg(Arg::with_name("Filename")
                    .takes_value(true)
                    .empty_values(false)
                    .multiple(true)
                    .required(true)
                )
            )
            .subcommand(SubCommand::with_name("edit")
                .about(fl!("repo-edit"))
                .arg(Arg::with_name("Filename")
                    .takes_value(true)
                    .empty_values(false)
                )
            )
            .subcommand(SubCommand::with_name("apply")
                .about(fl!("repo-apply"))
                .arg(Arg::with_name("Filename")
                    .takes_value(true)
                    .empty_values(false)
                    .required(true)
                )
            )
        )
        .get_matches();

    if let Some(matches) = matches.subcommand_matches("flac") {
        debug!("SubCommand matched: flac");
        flac::handle_flac(matches)?;
    } else if let Some(matches) = matches.subcommand_matches("cue") {
        debug!("SubCommand matched: cue");
        cue::handle_cue(matches)?;
    } else if let Some(matches) = matches.subcommand_matches("split") {
        debug!("SubCommand matched: split");
        split::handle_split(matches)?;
    } else if let Some(matches) = matches.subcommand_matches("repo") {
        debug!("SubCommand matched: repo");
        repo::handle_repo(matches)?;
    }

    Ok(())
}
