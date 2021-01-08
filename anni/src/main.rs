use clap::{App, Arg, ArgGroup, crate_authors, crate_version, SubCommand};
use anni_utils::fs;
use std::path::PathBuf;

mod flac;
mod encoding;
mod cue;

#[macro_use]
extern crate lazy_static;

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
            .arg(Arg::with_name("flac.tag.check")
                .long("check")
                .short("c")
            )
            .group(ArgGroup::with_name("group.flac").args(&["flac.list", "flac.tags"]))
        )
        .subcommand(SubCommand::with_name("cue")
            .arg(Arg::with_name("cue.file")
                .long("file")
                .short("f")
                .takes_value(true)
                .required(true)
            )
            .arg(Arg::with_name("cue.tagsh")
                .long("tag-sh")
                .short("t")
            )
        )
        .subcommand(SubCommand::with_name("repo")
            .arg(Arg::with_name("repo.new_album")
                .long("new-album")
                .short("n")
            )
        )
        .subcommand(SubCommand::with_name("play"))
        .subcommand(SubCommand::with_name("versary"))
        .arg(Arg::with_name("Filename")
            .index(1)
            .takes_value(true)
            .empty_values(false)
            .multiple(true)
            .global(true)
        )
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
                    if matches.is_present("flac.tag.check") {
                        flac::tags_check(name, stream);
                    } else {
                        flac::tags(stream);
                    }
                    !pwd_used
                });
            }
        }
    } else if let Some(matches) = matches.subcommand_matches("cue") {
        let cue_file = matches.value_of("cue.file").unwrap();
        if matches.is_present("cue.tagsh") {
            if let Some(files) = matches.values_of("Filename") {
                let files: Vec<_> = files.collect();
                let result = cue::parse_file(cue_file, &files).ok_or("Failed to parse CUE file.")?;
                println!("{}", result);
            }
        }
    } else if let Some(matches) = matches.subcommand_matches("repo") {} else if let Some(_matches) = matches.subcommand_matches("versary") {
        let _ = anni_versary::anni_versary();
    }

    Ok(())
}
