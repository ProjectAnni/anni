use anni_flac::{MetadataBlockData, Stream, parse_flac};
use std::fs::File;
use std::io::Read;
use crate::{encoding, fs};
use std::path::{PathBuf, Path};
use anni_utils::validator::{Validator, trim_validator, date_validator, number_validator, artist_validator};
use std::collections::HashSet;
use colored::*;

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
    fs::walk_path(PathBuf::from(input), true, |file| {
        // ignore non-flac files
        match file.extension() {
            None => return true,
            Some(ext) => {
                if ext != "flac" {
                    return true;
                }
            }
        };

        let filename = file.to_str().unwrap();
        let stream = parse_file(filename);
        match stream {
            Ok(stream) => callback(filename, &stream),
            Err(err) => {
                println!("{}", err);
                false
            }
        }
    }).unwrap_or_else(|e| panic!(e));
}

pub(crate) fn info_list(stream: &Stream) {
    for (i, block) in stream.metadata_blocks.iter().enumerate() {
        println!("METADATA block #{}", i);
        println!("  type: {} ({1})", u8::from(&block.data), block.data.to_string());
        println!("  is last: {}", block.is_last);
        println!("  length: {}", block.length);
        match &block.data {
            MetadataBlockData::StreamInfo(s) => {
                println!("  minimum blocksize: {} samples", s.min_block_size);
                println!("  maximum blocksize: {} samples", s.max_block_size);
                println!("  minimum framesize: {} bytes", s.min_frame_size);
                println!("  maximum framesize: {} bytes", s.max_frame_size);
                println!("  sample_rate: {} Hz", s.sample_rate);
                println!("  channels: {}", s.channels);
                println!("  bits-per-sample: {}", s.bits_per_sample);
                println!("  total samples: {}", s.total_samples);
                println!("  MD5 signature: {}", hex::encode(s.md5_signature));
            }
            MetadataBlockData::Application(s) => {
                println!("  application ID: {:x}", s.application_id);
                println!("  data contents:");
                // TODO: hexdump
                println!("  <TODO>");
            }
            MetadataBlockData::SeekTable(s) => {
                println!("  seek points: {}", s.seek_points.len());
                for (i, p) in s.seek_points.iter().enumerate() {
                    if p.is_placehoder() {
                        println!("    point {}: PLACEHOLDER", i);
                    } else {
                        println!("    point {}: sample_number={}, stream_offset={}, frame_samples={}", i, p.sample_number, p.stream_offset, p.frame_samples);
                    }
                }
            }
            MetadataBlockData::VorbisComment(s) => {
                println!("  vendor string: {}", s.vendor_string);
                println!("  comments: {}", s.len());
                for (i, (key, c)) in s.comments.iter().enumerate() {
                    println!("    comment[{}]: {}={}", i, key, c.value());
                }
            }
            MetadataBlockData::CueSheet(s) => {
                println!("  media catalog number: {}", s.catalog_number);
                println!("  lead-in: {}", s.leadin_samples);
                println!("  is CD: {}", s.is_cd);
                println!("  number of tracks: {}", s.track_number);
                for (i, t) in s.tracks.iter().enumerate() {
                    println!("    track[{}]", i);
                    println!("      offset: {}", t.track_offset);
                    // TODO: https://github.com/xiph/flac/blob/ce6dd6b5732e319ef60716d9cc9af6a836a4011a/src/metaflac/operations.c#L627-L651
                }
            }
            MetadataBlockData::Picture(s) => {
                println!("  type: {} ({})", u8::from(&s.picture_type), s.picture_type.to_string());
                println!("  MIME type: {}", s.mime_type);
                println!("  description: {}", s.description);
                println!("  width: {}", s.width);
                println!("  height: {}", s.height);
                println!("  depth: {}", s.depth);
                println!("  colors: {}{}", s.colors, if s.color_indexed() { "" } else { " (unindexed)" });
                println!("  data length: {}", s.data_length);
                println!("  data:");
                // TODO: hexdump
                println!("  <TODO>");
            }
            _ => {}
        }
    }
}

pub(crate) fn tags(stream: &Stream) {
    for block in stream.metadata_blocks.iter() {
        match &block.data {
            MetadataBlockData::VorbisComment(s) => {
                for (key, c) in s.comments.iter() {
                    println!("{}={}", key, c.value());
                }
                break;
            }
            _ => {}
        }
    }
}

macro_rules! init_hasproblem {
    ($has_problem: ident, $filename: expr) => {
        if !$has_problem {
            eprintln!("## File {}:", $filename);
            $has_problem = true;
        }
    };
}

pub(crate) fn tags_check(filename: &str, stream: &Stream) {
    for block in stream.metadata_blocks.iter() {
        match &block.data {
            MetadataBlockData::VorbisComment(s) => {
                let mut has_problem = false;
                for tag in TAG_REQUIREMENT.iter() {
                    match tag {
                        FlacTag::Must(key, validator) => {
                            if !s.comments.contains_key(*key) {
                                init_hasproblem!(has_problem, filename);
                                eprintln!("- Missing tag: {}", key.green());
                            } else {
                                let key_raw: &str = &s.comments[*key].key_raw();
                                let value: &str = &s.comments[*key].value();
                                if !validator(&value) {
                                    init_hasproblem!(has_problem, filename);
                                    eprintln!("- Invalid value for tag {}: {}", key_raw.green(), value.red());
                                }
                            }
                        }
                        FlacTag::Optional(key, validator) => {
                            if s.comments.contains_key(*key) {
                                let key_raw: &str = &s.comments[*key].key_raw();
                                let value: &str = &s.comments[*key].value();
                                if !validator(&value) {
                                    init_hasproblem!(has_problem, filename);
                                    eprintln!("- Invalid value for optional tag {}: {}", key_raw.green(), value.red());
                                }
                            }
                        }
                        FlacTag::Unrecommended(key, alternative) => {
                            if s.comments.contains_key(*key) {
                                let value = s.comments[*key].value();
                                init_hasproblem!(has_problem, filename);
                                eprintln!("- Unrecommended tag: {}, use {} instead", key.red(), alternative.green());
                                println!("metaflac --remove-tag={} --set-tag='{}={}' '{}'", key, alternative, value, filename);
                            }
                        }
                    }
                }

                let mut key_set: HashSet<String> = HashSet::new();
                for (key, comment) in s.comments.iter() {
                    let key: &str = key;
                    let key_raw: &str = &comment.key_raw();
                    let value: &str = &comment.value();
                    let entry: &str = &comment.entry();

                    if key_set.contains(key) {
                        init_hasproblem!(has_problem, filename);
                        eprintln!("- Duplicated tag: {}", key.red());
                        continue;
                    } else if !TAG_INCLUDED.contains(&key) {
                        init_hasproblem!(has_problem, filename);
                        eprintln!("- Unnecessary tag: {}", key.red());
                        println!("metaflac --remove-tag={} '{}'", key, filename);
                        continue;
                    } else {
                        key_set.insert(key.to_string());
                    }

                    if !encoding::middle_dot_valid(&value) {
                        init_hasproblem!(has_problem, filename);
                        eprintln!("- Invalid middle dot in: {}", entry.red());
                        println!("metaflac --remove-tag={} --set-tag='{}={}' '{}'", key, key, encoding::middle_dot_replace(&value), filename);
                    }
                    if value.len() == 0 {
                        init_hasproblem!(has_problem, filename);
                        eprintln!("- Empty value for tag: {}", key.red());
                        println!("metaflac --remove-tag={} '{}'", key, filename);
                    }
                    if !comment.is_key_uppercase() {
                        init_hasproblem!(has_problem, filename);
                        eprintln!("- Tag in lowercase: {}", key_raw.red());
                        println!("metaflac --remove-tag={} --set-tag='{}={}' '{}'", key_raw, key, value, filename);
                    }
                }

                // Filename check
                if s.comments.contains_key("TRACKNUMBER") && s.comments.contains_key("TITLE") {
                    let mut number = s.comments["TRACKNUMBER"].value();
                    if number.len() == 1 {
                        number = format!("0{}", number);
                    }
                    let filename_expected: &str = &format!("{}. {}.flac", number, s.comments["TITLE"].value());
                    let filename_raw = Path::new(filename).file_name().unwrap().to_str().expect("Non-UTF8 filenames are currently not supported!");
                    if filename_raw != filename_expected {
                        init_hasproblem!(has_problem, filename);
                        eprintln!("- Filename mismatch: expected {}, got {}", filename_expected.green(), filename_raw.red());
                    }
                }
            }
            _ => {}
        }
    }
}