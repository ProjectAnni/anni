use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};

use anni_flac::blocks::PictureType;
use anni_utils::fs;
use anni_utils::validator::{artist_validator, date_validator, number_validator, trim_validator, Validator};

use crate::encoding;
use shell_escape::escape;
use anni_utils::fs::PathWalker;
use std::iter::FilterMap;
use anni_flac::{MetadataBlockData, FlacHeader};
use clap::ArgMatches;
use std::process::exit;

enum FlacTag {
    Must(&'static str, Validator),
    Optional(&'static str, Validator),
    Unrecommended(&'static str, &'static str),
}

impl ToString for FlacTag {
    fn to_string(&self) -> String {
        match self {
            FlacTag::Must(s, _) => s.to_string(),
            FlacTag::Optional(s, _) => s.to_string(),
            FlacTag::Unrecommended(s, _) => s.to_string(),
        }
    }
}

const TAG_REQUIREMENT: [FlacTag; 11] = [
    // MUST tags
    FlacTag::Must("TITLE", trim_validator),
    FlacTag::Must("ARTIST", artist_validator),
    FlacTag::Must("ALBUM", trim_validator),
    FlacTag::Must("DATE", date_validator),
    FlacTag::Must("TRACKNUMBER", number_validator),
    FlacTag::Must("TRACKTOTAL", number_validator),
    FlacTag::Optional("DISCNUMBER", number_validator),
    FlacTag::Optional("DISCTOTAL", number_validator),
    // OPTIONAL tags
    FlacTag::Optional("ALBUMARTIST", trim_validator),
    // UNRECOMMENDED tags with alternatives
    FlacTag::Unrecommended("TOTALTRACKS", "TRACKTOTAL"),
    FlacTag::Unrecommended("TOTALDISCS", "DISCTOTAL"),
];

const TAG_INCLUDED: [&'static str; 11] = [
    "TITLE", "ARTIST", "ALBUM", "DATE", "TRACKNUMBER", "TRACKTOTAL",
    "ALBUMARTIST", "DISCNUMBER", "DISCTOTAL",
    "TOTALTRACKS", "TOTALDISCS",
];

pub(crate) fn handle_flac(matches: &ArgMatches) -> anyhow::Result<()> {
    if matches.is_present("flac.check") {
        let pwd = PathBuf::from("./");
        let (paths, is_pwd) = match matches.values_of("Filename") {
            Some(files) => (files.collect(), false),
            None => (vec![pwd.to_str().expect("Failed to convert to str")], true),
        };
        for input in paths {
            for (file, header) in parse_input_iter(input) {
                if let Err(e) = header {
                    error!("Failed to parse header of {:?}: {:?}", file, e);
                    exit(1);
                }

                tags_check(file.to_string_lossy().as_ref(), &header.unwrap());
                if is_pwd {
                    break;
                }
            }
        }
    } else if matches.is_present("flac.export") {
        let mut files = if let Some(filename) = matches.value_of("Filename") {
            parse_input_iter(filename)
        } else {
            panic!("No filename provided.");
        };

        let (_, file) = files.nth(0).ok_or(anyhow!("No valid file found."))?;
        let file = file?;
        match matches.value_of("flac.export.type").unwrap() {
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
    }
    Ok(())
}

fn parse_input_iter(input: &str) -> FilterMap<PathWalker, fn(PathBuf) -> Option<(PathBuf, anni_flac::prelude::Result<FlacHeader>)>> {
    fs::PathWalker::new(PathBuf::from(input), true).filter_map(|file| {
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
                MetadataBlockData::Comment(s) => { println!("{}", s); }
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

fn tags_check(filename: &str, stream: &FlacHeader) {
    info!("Checking {}", filename);
    let mut fixes = Vec::new();
    let comments = stream.comments().expect("Failed to read comments");
    let map = comments.to_map();
    for tag in TAG_REQUIREMENT.iter() {
        match tag {
            FlacTag::Must(key, validator) => {
                if !map.contains_key(*key) {
                    error!("Missing tag: {}", key);
                } else {
                    let value = map[*key].value();
                    if !validator(&value) {
                        error!("Invalid tag value: {}={}", key, value);
                    }
                }
            }
            FlacTag::Optional(key, validator) => {
                if map.contains_key(*key) {
                    let value = map[*key].value();
                    if !validator(&value) {
                        warn!("Invalid optional tag value: {}={}", key, value);
                    }
                }
            }
            FlacTag::Unrecommended(key, alternative) => {
                if map.contains_key(*key) {
                    let value = map[*key].value();
                    warn!("Unrecommended key: {}={}, use {} instead", key, value, alternative);
                }
            }
        }
    }

    let mut key_set: HashSet<String> = HashSet::new();
    for (key, comment) in map.iter() {
        let key: &str = key;
        let key_raw = comment.key_raw();
        let value = comment.value();

        if key_set.contains(key) {
            warn!("Duplicated tag: {}", key);
            continue;
        } else if !TAG_INCLUDED.contains(&key) {
            warn!("Unnecessary tag: {}", key);
            fixes.push(format!("metaflac --remove-tag={} {}", escape(key.into()), escape(filename.into())));
            continue;
        } else {
            key_set.insert(key.to_string());
        }

        if !encoding::middle_dot_valid(&value) {
            let correct = encoding::middle_dot_replace(&value);
            warn!("Invalid middle dot in tag {}: {}", key, value);
            fixes.push(format!("metaflac --remove-tag={} --set-tag={} {}", escape(key.into()), escape(format!("{}={}", key, correct).into()), escape(filename.into())));
        }
        if value.len() == 0 {
            warn!("Empty value for tag: {}", key);
        }
        if !comment.is_key_uppercase() {
            warn!("Lowercase tag: {}", key_raw);
        }
    }

    // Filename check
    if map.contains_key("TRACKNUMBER") && map.contains_key("TITLE") {
        let mut number = map["TRACKNUMBER"].value().to_owned();
        if number.len() == 1 {
            number = format!("0{}", number);
        }
        let filename_expected: &str = &format!("{}. {}.flac", number, map["TITLE"].value()).replace("/", "／");
        let filename_raw = Path::new(filename).file_name().unwrap().to_str().expect("Non-UTF8 filenames are currently not supported!");
        if filename_raw != filename_expected {
            error!("Filename mismatch, got {}, expected {}", filename_raw, filename_expected);
            let path_expected = Path::new(filename).with_file_name(filename_expected);
            fixes.push(format!("mv {} {}", escape(filename.into()), escape(path_expected.to_string_lossy())));
        }
    }
    for fix in fixes.iter() {
        println!("{}", fix);
    }
}