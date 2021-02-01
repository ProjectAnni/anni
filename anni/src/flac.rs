use std::collections::HashSet;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anni_flac::{MetadataBlockData, parse_flac, Stream, PictureType};
use anni_utils::{fs, report};
use anni_utils::validator::{artist_validator, date_validator, number_validator, trim_validator, Validator};

use crate::encoding;
use shell_escape::escape;
use anni_utils::fs::PathWalker;
use std::iter::FilterMap;

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
    // OPTIONAL tags
    FlacTag::Optional("ALBUMARTIST", trim_validator),
    FlacTag::Optional("DISCNUMBER", number_validator),
    FlacTag::Optional("DISCTOTAL", number_validator),
    // UNRECOMMENDED tags with alternatives
    FlacTag::Unrecommended("TOTALTRACKS", "TRACKTOTAL"),
    FlacTag::Unrecommended("TOTALDISCS", "DISCTOTAL"),
];

const TAG_INCLUDED: [&'static str; 11] = [
    "TITLE", "ARTIST", "ALBUM", "DATE", "TRACKNUMBER", "TRACKTOTAL",
    "ALBUMARTIST", "DISCNUMBER", "DISCTOTAL",
    "TOTALTRACKS", "TOTALDISCS",
];

pub(crate) fn parse_file(filename: &str) -> Result<Stream, String> {
    let mut file = File::open(filename).expect(&format!("Failed to open file: {}", filename));
    let mut data = Vec::new();
    file.read_to_end(&mut data).expect(&format!("Failed to read file: {}", filename));
    parse_flac(&data, None).map_err(|o| o.to_string())
}

pub(crate) fn parse_input(input: &str, callback: impl Fn(&str, &Stream) -> bool) {
    for file in fs::PathWalker::new(PathBuf::from(input), true) {
        match file.extension() {
            None => continue,
            Some(ext) => {
                if ext != "flac" {
                    continue;
                }
            }
        };

        let filename = file.to_str().unwrap();
        let stream = parse_file(filename);
        match stream {
            Ok(stream) => if !callback(filename, &stream) {
                break;
            },
            Err(err) => {
                eprintln!("{}", err);
                break;
            }
        }
    }
}

pub(crate) fn parse_input_iter(input: &str) -> FilterMap<PathWalker, fn(PathBuf) -> Option<Stream>> {
    fs::PathWalker::new(PathBuf::from(input), true).filter_map(|file| {
        match file.extension() {
            None => return None,
            Some(ext) => {
                if ext != "flac" {
                    return None;
                }
            }
        };

        let filename = file.to_str()?;
        parse_file(filename).ok()
    })
}

pub(crate) fn info_list(stream: &Stream) {
    for (i, block) in stream.metadata_blocks.iter().enumerate() {
        block.print(i);
    }
}

pub(crate) enum ExportConfig {
    Cover(ExportConfigCover),
    None,
}

pub(crate) struct ExportConfigCover {
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

pub(crate) fn export(stream: &Stream, b: &str, export_config: ExportConfig) {
    let mut first_picture = true;
    for (i, block) in stream.metadata_blocks.iter().enumerate() {
        if block.data.as_str() == b {
            match &block.data {
                MetadataBlockData::VorbisComment(s) => { println!("{}", s.to_string()); }
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
                        should_export &= picture_type == &p.picture_type;
                    };
                    // Block num match
                    if let Some(block_num) = config.block_num {
                        should_export &= block_num == i;
                    }

                    if should_export {
                        let stdout = std::io::stdout();
                        let mut handle = stdout.lock();
                        handle.write_all(&p.data).unwrap();
                    }

                    // Only export the first picture
                    if first_picture {
                        first_picture = false;
                    }
                }
                _ => block.print(i),
            };
        }
    }
}

pub(crate) fn tags_check(filename: &str, stream: &Stream, report_mode: &str) {
    let mut reporter = report::new(report_mode);
    let mut fixes = Vec::new();
    for block in stream.metadata_blocks.iter() {
        match &block.data {
            MetadataBlockData::VorbisComment(s) => {
                for tag in TAG_REQUIREMENT.iter() {
                    match tag {
                        FlacTag::Must(key, validator) => {
                            if !s.comments.contains_key(*key) {
                                reporter.add_problem(filename, "Missing Tag", key, None, Some("Add"));
                            } else {
                                let value = s.comments[*key].value();
                                if !validator(&value) {
                                    reporter.add_problem(filename, "Invalid value", key, Some(value), Some("Replace"));
                                }
                            }
                        }
                        FlacTag::Optional(key, validator) => {
                            if s.comments.contains_key(*key) {
                                let value = s.comments[*key].value();
                                if !validator(&value) {
                                    reporter.add_problem(filename, "Invalid value", key, Some(value), Some("Replace / Remove"));
                                }
                            }
                        }
                        FlacTag::Unrecommended(key, alternative) => {
                            if s.comments.contains_key(*key) {
                                let value = s.comments[*key].value();
                                reporter.add_problem(filename, "Unrecommended tag", key, Some(value), Some(alternative));
                            }
                        }
                    }
                }

                let mut key_set: HashSet<String> = HashSet::new();
                for (key, comment) in s.comments.iter() {
                    let key: &str = key;
                    let key_raw: &str = &comment.key_raw();
                    let value = comment.value();

                    if key_set.contains(key) {
                        reporter.add_problem(filename, "Duplicated tag", key, None, Some("Remove"));
                        continue;
                    } else if !TAG_INCLUDED.contains(&key) {
                        fixes.push(format!("metaflac --remove-tag={} '{}'", key, filename));
                        reporter.add_problem(filename, "Unnecessary tag", key, Some(value), Some("Remove"));
                        continue;
                    } else {
                        key_set.insert(key.to_string());
                    }

                    if !encoding::middle_dot_valid(&value) {
                        let correct = encoding::middle_dot_replace(&value);
                        reporter.add_problem(filename, "Invalid middle dot", key, Some(value), Some(&correct));
                    }
                    if value.len() == 0 {
                        reporter.add_problem(filename, "Empty value", key, None, Some("Remove"));
                    }
                    if !comment.is_key_uppercase() {
                        reporter.add_problem(filename, "Lowercase tag", key_raw, Some(value), Some(key));
                    }
                }

                // Filename check
                if s.comments.contains_key("TRACKNUMBER") && s.comments.contains_key("TITLE") {
                    let mut number = s.comments["TRACKNUMBER"].value().to_owned();
                    if number.len() == 1 {
                        number = format!("0{}", number);
                    }
                    let filename_expected: &str = &format!("{}. {}.flac", number, s.comments["TITLE"].value());
                    let filename_raw = Path::new(filename).file_name().unwrap().to_str().expect("Non-UTF8 filenames are currently not supported!");
                    if filename_raw != filename_expected {
                        reporter.add_problem(filename, "Filename mismatch", filename_raw, None, Some(filename_expected));
                        fixes.push(format!("mv {} {}", escape(filename_raw.into()), escape(filename_expected.into())));
                    }
                }
            }
            _ => {}
        }
    }
    reporter.eprint();
    for fix in fixes.iter() {
        println!("{}", fix);
    }
}