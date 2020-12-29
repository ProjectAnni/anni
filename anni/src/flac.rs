use anni_flac::{MetadataBlockData, Stream, parse_flac};
use std::fs::File;
use std::io::Read;
use crate::{encoding, fs};
use std::path::PathBuf;

enum FlacTag {
    Must(&'static str),
    Optional(&'static str),
    Unrecommended(&'static str, &'static str),
}

impl ToString for FlacTag {
    fn to_string(&self) -> String {
        match self {
            FlacTag::Must(s) => s.to_string(),
            FlacTag::Optional(s) => s.to_string(),
            FlacTag::Unrecommended(s, _) => s.to_string(),
        }
    }
}

const TAG_REQUIREMENT: &[&FlacTag] = &[
    // MUST tags
    &FlacTag::Must("TITLE"),
    &FlacTag::Must("ARTIST"),
    &FlacTag::Must("ALBUM"),
    &FlacTag::Must("DATE"),
    &FlacTag::Must("TRACKNUMBER"),
    &FlacTag::Must("TRACKTOTAL"),
    // OPTIONAL tags
    &FlacTag::Optional("ALBUMARTIST"),
    &FlacTag::Optional("DISCNUMBER"),
    &FlacTag::Optional("DISCTOTAL"),
    // UNRECOMMENDED tags with alternatives
    &FlacTag::Unrecommended("TOTALTRACKS", "TRACKTOTAL"),
    &FlacTag::Unrecommended("TOTALDISCS", "DISCTOTAL"),
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
                for (i, c) in s.comments.iter().enumerate() {
                    println!("    comment[{}]: {}={}", i, c.key(), c.value());
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
                for c in s.comments.iter() {
                    println!("{}={}", c.key(), c.value());
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
                let mut needed_exist: [bool; 6] = [false; 6];
                for c in s.comments.iter() {
                    let key = c.key();
                    let value = c.value();
                    let entry = c.entry();
                    if !encoding::middle_dot_valid(&value) {
                        init_hasproblem!(has_problem, filename);
                        eprintln!("- Invalid middle dot in: {}", entry);
                        println!("metaflac --remove-tag={} --set-tag='{}={}' '{}'", key, key, encoding::middle_dot_replace(&value), filename);
                    }

                    match TAG_REQUIREMENT.iter().position(|&s| match s {
                        FlacTag::Must(s) => *s == &key,
                        FlacTag::Optional(s) => *s == &key,
                        FlacTag::Unrecommended(s, _) => *s == &key,
                    }) {
                        Some(tag) => {
                            match TAG_REQUIREMENT[tag] {
                                FlacTag::Must(_) | FlacTag::Optional(_) => {
                                    if value.len() == 0 {
                                        init_hasproblem!(has_problem, filename);
                                        eprintln!("- Empty value for tag: {}", key);
                                        println!("metaflac --remove-tag={} '{}'", key, filename);
                                    } else if !c.is_key_uppercase() {
                                        let key_raw = c.key_raw();
                                        init_hasproblem!(has_problem, filename);
                                        eprintln!("- Tag in lowercase: {}", key_raw);
                                        println!("metaflac --remove-tag={} --set-tag='{}={}' '{}'", key_raw, key, value, filename);
                                    }

                                    if tag < needed_exist.len() {
                                        if needed_exist[tag] {
                                            init_hasproblem!(has_problem, filename);
                                            eprintln!("- Duplicated tag: {}", key);
                                        }
                                        needed_exist[tag] = true;
                                    }
                                }
                                FlacTag::Unrecommended(_, alternative) => {
                                    init_hasproblem!(has_problem, filename);
                                    eprintln!("- Unrecommended tag: {}, use {} instead", key, alternative);
                                    println!("metadata --remove-tag={} --set-tag='{}={}' '{}'", key, alternative, value, filename);
                                }
                            }
                        }
                        None => {
                            init_hasproblem!(has_problem, filename);
                            eprintln!("- Unnecessary tag: {}", key);
                            println!("metaflac --remove-tag={} '{}'", key, filename);
                        }
                    }
                }
                for (i, exist) in needed_exist.iter().enumerate() {
                    if !exist {
                        init_hasproblem!(has_problem, filename);
                        eprintln!("- Missing tag: {}", TAG_REQUIREMENT[i].to_string());
                    }
                }
                break;
            }
            _ => {}
        }
    }
}