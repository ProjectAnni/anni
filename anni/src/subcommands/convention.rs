use clap::{ArgMatches, App, Arg};
use crate::subcommands::Subcommand;
use crate::{fl, encoding};
use std::path::Path;
use crate::subcommands::flac::parse_input_iter;
use shell_escape::escape;
use std::collections::HashSet;
use anni_flac::FlacHeader;
use anni_utils::validator::*;

pub(crate) struct ConventionSubcommand;

impl Subcommand for ConventionSubcommand {
    fn name(&self) -> &'static str {
        "convention"
    }

    fn create(&self) -> App<'static> {
        App::new("convention")
            .about(fl!("convention"))
            .alias("conv")
            .subcommand(App::new("check")
                .about(fl!("convention-check"))
                .arg(Arg::new("Filename")
                    .takes_value(true)
                    .required(true)
                    .min_values(1)
                )
            )
    }

    fn handle(&self, matches: &ArgMatches) -> anyhow::Result<()> {
        if let Some(matches) = matches.subcommand_matches("check") {
            for input in matches.values_of_os("Filename").unwrap().collect::<Vec<_>>() {
                for (file, header) in parse_input_iter(input) {
                    if let Err(e) = header {
                        error!("Failed to parse header of {:?}: {:?}", file, e);
                        continue;
                    }

                    tags_check(file.to_string_lossy().as_ref(), &header.unwrap());
                }
            }
        }
        Ok(())
    }
}


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

    let mut key_set: HashSet<String> = Default::default();
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
