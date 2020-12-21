mod flac;

use std::fs::File;
use std::io::Read;
use clap::{Arg, App, SubCommand, crate_version, crate_authors, AppSettings, ArgGroup};
use anni_flac::{parse_flac};

fn main() {
    let matches = App::new("Project Annivers@ry")
        .version(crate_version!())
        .author(crate_authors!())
        .subcommand(SubCommand::with_name("flac")
            .arg(Arg::with_name("flac.list")
                .long("list")
                .short("l")
                .requires("Filename")
            )
            .arg(Arg::with_name("flac.tags")
                .long("tags")
                .short("t")
                .requires("Filename")
            )
            .arg(Arg::with_name("flac.insert")
                .long("insert")
                .short("i")
                .alias("add")
                .takes_value(true)
                .multiple(true)
            )
            .arg(Arg::with_name("flac.edit")
                .long("edit")
                .short("e")
                .takes_value(true)
                .multiple(true)
            )
            .arg(Arg::with_name("flac.delete")
                .long("delete")
                .short("d")
                .alias("remove")
                .takes_value(true)
                .multiple(true)
            )
            .group(ArgGroup::with_name("group.flac").args(&["flac.list", "flac.tags"]))
            .group(ArgGroup::with_name("group.flac.tags").args(&["flac.tags", "flac.insert", "flac.edit", "flac.delete"]).multiple(true))
        )
        .subcommand(SubCommand::with_name("split")
            .arg(Arg::with_name("split.cue")
                .long("cue")
                .short("c")
                .requires("Filename")
            )
        )
        .arg(Arg::with_name("Filename").index(1).takes_value(true).multiple(true).global(true))
        .setting(AppSettings::ColoredHelp)
        .get_matches();

    if let Some(matches) = matches.subcommand_matches("flac") {
        if let Some(files) = matches.values_of("Filename") {
            for filename in files {
                let mut file = File::open(filename).expect(&format!("Failed to open file: {}", filename));
                let mut data = Vec::new();
                file.read_to_end(&mut data).expect(&format!("Failed to read file: {}", filename));
                let (_, stream) = parse_flac(&data).unwrap();

                if matches.is_present("flac.list") {
                    flac::info_list(stream);
                } else if matches.is_present("flac.tags") {
                    if matches.is_present("group.flac.tags") {
                        // TODO: handle input in order
                        println!("{}", matches.value_of("flac.insert").unwrap())
                    } else {
                        flac::tags(stream);
                    }
                }
            }
        }
    }
}
