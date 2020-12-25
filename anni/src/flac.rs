use anni_flac::{MetadataBlockData, Stream, parse_flac};
use std::fs::File;
use std::io::Read;
use crate::{encoding, fs};
use std::path::PathBuf;

const MUST_TAGS: &[&str] = &["TITLE", "ARTIST", "ALBUM", "DATE", "TRACKNUMBER", "TRACKTOTAL"];
const OPTIONAL_TAGS: &[&str] = &["ALBUMARTIST", "DISCNUMBER", "DISCTOTAL"];
const UNRECOMMENDED_TAGS: &[(&str, &str)] = &[("TOTALTRACKS", "TRACKTOTAL"), ("TOTALDISCS", "DISCTOTAL")];

pub(crate) fn parse_file(filename: &str) -> Result<Stream, String> {
    let mut file = File::open(filename).expect(&format!("Failed to open file: {}", filename));
    let mut data = Vec::new();
    file.read_to_end(&mut data).expect(&format!("Failed to read file: {}", filename));
    parse_flac(&data).map_err(|o| o.to_string())
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
                println!("  comments: {}", s.comment_number);
                for (i, c) in s.comments.iter().enumerate() {
                    println!("    comment[{}]: {}={}", i, c.comment_key, c.comment_value);
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
                    println!("{}={}", c.comment_key, c.comment_value);
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
            println!("## File {}:", $filename);
            $has_problem = true;
        }
    };
}

pub(crate) fn tags_check(filename: &str, stream: &Stream) {
    for block in stream.metadata_blocks.iter() {
        match &block.data {
            MetadataBlockData::VorbisComment(s) => {
                let mut has_problem = false;
                let mut needed: [bool; 6] = [false; 6];
                for c in s.comments.iter() {
                    if c.comment_key != c.comment_key.to_ascii_uppercase() {
                        init_hasproblem!(has_problem, filename);
                        println!("- [Warning] Tag in lowercase: {}", c.comment_key);
                    }

                    if c.comment_value.len() == 0 {
                        init_hasproblem!(has_problem, filename);
                        println!("- Empty value for tag: {}", c.comment_key);
                    }

                    if let Some(dot) = encoding::middle_dot_valid(&c.comment_value) {
                        init_hasproblem!(has_problem, filename);
                        println!("- Invalid middle dot `{}` in: {}={}", &dot, c.comment_key, c.comment_value);
                    }

                    match MUST_TAGS.iter().position(|&s| s == c.comment_key) {
                        Some(i) => {
                            if needed[i] {
                                init_hasproblem!(has_problem, filename);
                                println!("- Duplicated tag: {}", c.comment_key);
                            }
                            needed[i] = true;
                        }
                        None => {
                            if UNRECOMMENDED_TAGS.iter().all(|(k, i)| {
                                if k == &c.comment_key {
                                    init_hasproblem!(has_problem, filename);
                                    println!("- Unrecommended tag: {}, use {} instead", c.comment_key, i);
                                    false
                                } else {
                                    true
                                }
                            }) {
                                if !OPTIONAL_TAGS.contains(&&*c.comment_key) {
                                    init_hasproblem!(has_problem, filename);
                                    println!("- Unnecessary tag: {}", c.comment_key);
                                }
                            }
                        }
                    }
                }
                for (i, val) in needed.iter().enumerate() {
                    if !val {
                        init_hasproblem!(has_problem, filename);
                        println!("- Missing tag: {}", MUST_TAGS[i]);
                    }
                }
                break;
            }
            _ => {}
        }
    }
}