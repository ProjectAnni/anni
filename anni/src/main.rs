mod flac;
mod encoding;
mod fs;

use clap::{Arg, App, SubCommand, crate_version, crate_authors, AppSettings, ArgGroup};

fn main() -> Result<(), String> {
    let matches = App::new("Project Annivers@ry")
        .version(crate_version!())
        .author(crate_authors!())
        .subcommand(SubCommand::with_name("flac")
            .arg(Arg::with_name("flac.list")
                .long("list")
                .short("l")
            )
            .arg(Arg::with_name("flac.tags")
                .long("tags")
                .short("t")
            )
            .arg(Arg::with_name("flac.insert")
                .long("insert")
                .short("i")
                .alias("add")
                .takes_value(true)
                .empty_values(false)
                .multiple(true)
            )
            .arg(Arg::with_name("flac.edit")
                .long("edit")
                .short("e")
                .takes_value(true)
                .empty_values(false)
                .multiple(true)
            )
            .arg(Arg::with_name("flac.delete")
                .long("delete")
                .short("d")
                .alias("remove")
                .takes_value(true)
                .empty_values(false)
                .multiple(true)
            )
            .arg(Arg::with_name("flac.tag.check")
                .long("check")
                .short("c")
            )
            .group(ArgGroup::with_name("group.flac").args(&["flac.list", "flac.tags"]))
            .group(ArgGroup::with_name("group.flac.operation").args(&["flac.insert", "flac.edit", "flac.delete"]).multiple(true))
        )
        .subcommand(SubCommand::with_name("split")
            .arg(Arg::with_name("split.cue")
                .long("cue")
                .short("c")
            )
        )
        .subcommand(SubCommand::with_name("play"))
        .arg(Arg::with_name("Filename")
            .index(1)
            .takes_value(true)
            .empty_values(false)
            .multiple(true)
            .global(true)
        )
        .setting(AppSettings::ColoredHelp)
        .get_matches();

    if let Some(matches) = matches.subcommand_matches("flac") {
        if matches.is_present("flac.list") {
            if let Some(files) = matches.values_of("Filename") {
                for filename in files {
                    flac::parse_input(filename, |_name, stream| {
                        flac::info_list(stream);
                        true
                    });
                }
            }
        } else if matches.is_present("flac.tags") {
            let pwd = std::env::current_dir().map_err(|e| e.to_string())?;
            let mut pwd_used = false;
            let files = match matches.values_of("Filename") {
                Some(files) => files.collect(),
                None => {
                    pwd_used = true;
                    vec![pwd.to_str().expect("Failed to convert to str")]
                }
            };
            for filename in files {
                flac::parse_input(filename, |name, stream| {
                    if matches.is_present("group.flac.operation") {
                        // TODO: handle input in order
                        println!("{}", matches.value_of("flac.insert").unwrap());
                        true
                    } else if matches.is_present("flac.tag.check") {
                        flac::tags_check(name, stream);
                        !pwd_used
                    } else {
                        flac::tags(stream);
                        !pwd_used
                    }
                });
            }
        }
    } else if let Some(matches) = matches.subcommand_matches("play") {
        if let Some(_files) = matches.values_of("Filename") {
            // TODO
        }
    }

    Ok(())
}
