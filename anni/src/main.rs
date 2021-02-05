use clap::{App, Arg, ArgGroup, crate_authors, crate_version, SubCommand, AppSettings};
use anni_utils::fs;
use std::path::PathBuf;
use shell_escape::escape;
use anni_flac::PictureType;
use crate::flac::{ExportConfig, ExportConfigCover};

mod flac;
mod encoding;
mod cue;
mod i18n;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = App::new("Project Annivers@ry")
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
            .arg(Arg::with_name("split.audio")
                .help(fl!("split-audio-format"))
                .long("audio")
                .short("a")
                .takes_value(true)
                .default_value("wav")
            )
            .arg(Arg::with_name("split.cover")
                .help(fl!("split-cover"))
                .long("cover")
                .short("c")
                .takes_value(true)
                .default_value("cover.jpg")
            )
            .arg(Arg::with_name("Filename")
                .takes_value(true)
                .empty_values(false)
                .multiple(true)
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
        .get_matches();

    if let Some(matches) = matches.subcommand_matches("flac") {
        if matches.is_present("flac.check") {
            let pwd = PathBuf::from("./");
            let (paths, is_pwd) = match matches.values_of("Filename") {
                Some(files) => (files.collect(), false),
                None => (vec![pwd.to_str().expect("Failed to convert to str")], true),
            };
            for input in paths {
                flac::parse_input(input, |name, stream| {
                    flac::tags_check(name, stream, matches.value_of("flac.report.format").unwrap());
                    !is_pwd // if is_pwd { false } else { true }
                });
            }
        } else if matches.is_present("flac.export") {
            let mut files = if let Some(filename) = matches.value_of("Filename") {
                flac::parse_input_iter(filename)
            } else {
                panic!("No filename provided.");
            };

            let file = files.nth(0).ok_or("No valid file found.")?;
            match matches.value_of("flac.export.type").unwrap() {
                "info" => flac::export(&file, "STREAMINFO", ExportConfig::None),
                "application" => flac::export(&file, "APPLICATION", ExportConfig::None),
                "seektable" => flac::export(&file, "SEEKTABLE", ExportConfig::None),
                "cue" => flac::export(&file, "CUESHEET", ExportConfig::None),
                "comment" | "tag" => flac::export(&file, "VORBIS_COMMENT", ExportConfig::None),
                "picture" =>
                    flac::export(&file, "PICTURE", ExportConfig::Cover(ExportConfigCover::default())),
                "cover" =>
                    flac::export(&file, "PICTURE", ExportConfig::Cover(ExportConfigCover {
                        picture_type: Some(PictureType::CoverFront),
                        block_num: None,
                    })),
                "list" | "all" => flac::info_list(&file),
                _ => panic!("Unknown export type.")
            }
        }
    } else if let Some(matches) = matches.subcommand_matches("cue") {
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
    } else if let Some(matches) = matches.subcommand_matches("split") {
        let audio_format = matches.value_of("split.audio").unwrap();
        if let Some(dir) = matches.value_of("Filename") {
            let path = PathBuf::from(dir);
            let cue = fs::get_ext_file(&path, "cue", false)
                .map_err(|e| e.to_string())?
                .map(|p| p.to_str().unwrap().to_owned())
                .ok_or("Failed to find CUE sheet.")?;
            let audio = fs::get_ext_file(&path, audio_format, false)
                .map_err(|e| e.to_string())?
                .map(|p| p.to_str().unwrap().to_owned())
                .ok_or("Failed to find audio file.")?;

            let cover = matches.value_of("split.cover").unwrap();
            println!(r#"shnsplit -f {} -o "flac flac --picture {} -o %f -" {} -t "%n. %t""#, escape(cue.into()), cover, escape(audio.into()));
        }
    } else if let Some(_matches) = matches.subcommand_matches("versary") {
        if cfg!(feature = "server") {
            unimplemented!();
        }
        let _ = anni_versary::launch();
    }

    Ok(())
}
