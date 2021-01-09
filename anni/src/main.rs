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
            )
            .arg(Arg::with_name("cue.dir")
                .long("dir")
                .short("d")
            )
            .group(ArgGroup::with_name("cue.source")
                .args(&["cue.file", "cue.dir"])
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
        fn get_cue_file(dir: &str) -> Option<String> {
            if fs::is_dir(dir).unwrap() {
                for file in fs::PathWalker::new(PathBuf::from(dir), false) {
                    let ext = file.extension().unwrap_or("".as_ref()).to_str().unwrap_or("");
                    if ext == "cue" {
                        return Some(file.to_str().expect("Invalid path.").to_owned());
                    }
                }
            }
            None
        }
        let (cue, files) = if matches.is_present("cue.file") {
            // In file mode, the path of CUE file is specified by -f
            // And all the files in <Filename> are FLAC files
            let c = matches.value_of("cue.file").map(|u| u.to_owned());
            let f = matches.values_of("Filename").map(
                |u| u.map(|v| v.to_owned()).collect::<Vec<_>>()
            );
            (c, f)
        } else if matches.is_present("cue.dir") && matches.is_present("Filename") {
            // In directory mode, only one path is used: <Filename>[0]
            // The first CUE file found in that directory is treated as CUE input
            // All other FLAC file in that directory are treated as input
            let dir = matches.value_of("Filename").expect("No filename provided.");
            let c = fs::get_ext_file(PathBuf::from(dir), "cue", false)
                .map_err(|e| e.to_string())?
                .map(|p| p.to_str().unwrap().to_owned());
            let f = fs::get_ext_files(PathBuf::from(dir), "flac", false)
                .map_err(|e| e.to_string())?
                .map(|p| p.iter().map(|t| t.to_str().unwrap().to_owned()).collect::<Vec<_>>());
            (c, f)
        } else {
            (None, None)
        };

        if let Some(cue) = cue {
            if let Some(files) = files {
                if matches.is_present("cue.tagsh") {
                    let result = cue::parse_file(&cue, &files).map_err(|e| e.to_string())?;
                    println!("{}", result);
                }
            }
        }
    } else if let Some(matches) = matches.subcommand_matches("repo") {} else if let Some(_matches) = matches.subcommand_matches("versary") {
        let _ = anni_versary::anni_versary();
    }

    Ok(())
}
