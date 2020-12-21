use anni_flac::{MetadataBlockData, Stream, parse_flac};
use std::fs;
use std::fs::File;
use std::io::Read;
use std::ops::Add;
use crate::encoding;

const MUST_TAGS: &[&str] = &["TITLE", "ARTIST", "ALBUM", "DATE", "TRACKNUMBER", "TRACKTOTAL", "DISCNUMBER", "DISCTOTAL"];
const OPTIONAL_TAGS: &[&str] = &["PERFORMER"];
const UNRECOMMENDED_TAGS: &[(&str, usize)] = &[("TOTALTRACKS", 5), ("TOTALDISCS", 7)];

pub(crate) fn parse_file(filename: &str) -> Result<Stream, String> {
    let mut file = File::open(filename).expect(&format!("Failed to open file: {}", filename));
    let mut data = Vec::new();
    file.read_to_end(&mut data).expect(&format!("Failed to read file: {}", filename));
    parse_flac(&data).map_err(|o| o.to_string())
}

pub(crate) fn parse_input(input: &str, callback: impl Fn(&str, &Stream)) {
    if let Ok(meta) = fs::metadata(input) {
        if meta.is_dir() {
            for file in glob::glob(&input.to_string().add("/**/*.flac")).unwrap() {
                let filename = file.unwrap().to_str().unwrap().to_string();
                if let Ok(stream) = parse_file(&filename) {
                    callback(&filename, &stream);
                }
            }
        } else if meta.is_file() {
            if let Ok(stream) = parse_file(input) {
                callback(input, &stream);
            }
        }
    }
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
                    println!("    comment[{}]: {}", i, c.comment);
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
                    println!("{}", c.comment);
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
                let mut needed: [bool; 8] = [false; 8];
                for c in s.comments.iter() {
                    let mut splitter = c.comment.splitn(2, "=");
                    let key = splitter.next().unwrap().to_ascii_uppercase();
                    match splitter.next() {
                        Some(val) => {
                            if let Some(dot) = encoding::middle_dot_valid(val) {
                                init_hasproblem!(has_problem, filename);
                                println!("- Invalid middle dot `{}` in: {}={}", &dot, key, val);
                            }
                        }
                        None => {
                            init_hasproblem!(has_problem, filename);
                            println!("- Empty value for tag: {}", key);
                        }
                    };
                    match MUST_TAGS.iter().position(|&s| s == key) {
                        Some(i) => {
                            if needed[i] {
                                init_hasproblem!(has_problem, filename);
                                println!("- Duplicated tag: {}", key);
                            }
                            needed[i] = true;
                        }
                        None => {
                            if UNRECOMMENDED_TAGS.iter().all(|(k, i)| {
                                if k == &key {
                                    init_hasproblem!(has_problem, filename);
                                    println!("- Unrecommended tag: {}, use {} instead", key, MUST_TAGS[*i]);
                                    false
                                } else {
                                    true
                                }
                            }) {
                                if !OPTIONAL_TAGS.contains(&&*key) {
                                    init_hasproblem!(has_problem, filename);
                                    println!("- Unnecessary tag: {}", key);
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