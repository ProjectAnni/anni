use std::io::Write;
use std::path::{PathBuf, Path};
use anni_flac::blocks::PictureType;
use anni_flac::{MetadataBlockData, FlacHeader};
use clap::{ArgMatches, App, Arg};
use crate::subcommands::Subcommand;
use std::iter::FilterMap;
use anni_common::fs::PathWalker;
use anni_common::fs;
use crate::i18n::ClapI18n;

pub(crate) struct FlacSubcommand;

impl Subcommand for FlacSubcommand {
    fn name(&self) -> &'static str {
        "flac"
    }

    fn create(&self) -> App<'static> {
        App::new("flac")
            .about_ll("flac")
            .subcommand(App::new("export")
                .about_ll("flac-export")
                .arg(Arg::new("type")
                    .about_ll("flac-export-type")
                    .long("type")
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
                .arg(Arg::new("output")
                    .about_ll("flac-export-to")
                    .long("output")
                    .short('o')
                    .takes_value(true)
                    .default_value("-")
                )
                .arg(Arg::new("Filename")
                    .takes_value(true)
                    .required(true)
                    .min_values(1)
                )
            )
    }

    fn handle(&self, matches: &ArgMatches) -> anyhow::Result<()> {
        if let Some(matches) = matches.subcommand_matches("export") {
            let mut count = 0;
            for (_, file) in matches.value_of("Filename").map(|f| parse_input_iter(f)).unwrap() {
                let file = file?;
                match matches.value_of("type").unwrap() {
                    "info" => export(&file, "STREAMINFO", ExportConfig::None),
                    "application" => export(&file, "APPLICATION", ExportConfig::None),
                    "seektable" => export(&file, "SEEKTABLE", ExportConfig::None),
                    "cue" => export(&file, "CUESHEET", ExportConfig::None),
                    "comment" | "tag" => export(&file, "VORBIS_COMMENT", ExportConfig::None),
                    "picture" =>
                        export(&file, "PICTURE", ExportConfig::Cover(ExportConfigCover::default())),
                    "cover" =>
                        export(&file, "PICTURE", ExportConfig::Cover(ExportConfigCover {
                            picture_type: Some(PictureType::CoverFront),
                            block_num: None,
                        })),
                    "list" | "all" => info_list(&file),
                    _ => panic!("Unknown export type.")
                }
                count += 1;
            }

            if count == 0 {
                warn!("No flac file found.")
            }
        }
        Ok(())
    }
}

pub(crate) fn parse_input_iter<P: AsRef<Path>>(input: P) -> FilterMap<PathWalker, fn(PathBuf) -> Option<(PathBuf, anni_flac::prelude::Result<FlacHeader>)>> {
    fs::PathWalker::new(input, true).filter_map(|file| {
        match file.extension() {
            None => return None,
            Some(ext) => {
                if ext != "flac" {
                    return None;
                }
            }
        };

        let header = FlacHeader::from_file(&file);
        Some((file, header))
    })
}

fn info_list(stream: &FlacHeader) {
    for (i, block) in stream.blocks.iter().enumerate() {
        block.print(i);
    }
}

enum ExportConfig {
    Cover(ExportConfigCover),
    None,
}

struct ExportConfigCover {
    pub(crate) picture_type: Option<PictureType>,
    pub(crate) block_num: Option<usize>,
}

impl Default for ExportConfigCover {
    fn default() -> Self {
        ExportConfigCover {
            picture_type: None,
            block_num: None,
        }
    }
}

fn export(header: &FlacHeader, b: &str, export_config: ExportConfig) {
    let mut first_picture = true;
    for (i, block) in header.blocks.iter().enumerate() {
        if block.data.as_str() == b {
            match &block.data {
                MetadataBlockData::Comment(s) => { print!("{}", s); }
                MetadataBlockData::CueSheet(_) => {} // TODO
                MetadataBlockData::Picture(p) => {
                    // Load config
                    let config = match &export_config {
                        ExportConfig::Cover(c) => c,
                        _ => unreachable!(),
                    };

                    let mut should_export = first_picture;
                    // PictureType match
                    if let Some(picture_type) = &config.picture_type {
                        should_export &= (p.picture_type as u8) == (*picture_type as u8);
                    };
                    // Block num match
                    if let Some(block_num) = config.block_num {
                        should_export &= block_num == i;
                    }

                    if should_export {
                        let stdout = std::io::stdout();
                        let mut handle = stdout.lock();
                        handle.write_all(&p.data).unwrap();

                        // Only export the first picture
                        if first_picture {
                            first_picture = false;
                        }
                    }
                }
                _ => block.print(i),
            };
        }
    }
}
