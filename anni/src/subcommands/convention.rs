use clap::{ArgMatches, App, Arg};
use crate::subcommands::Subcommand;
use crate::subcommands::flac::parse_input_iter;
use crate::i18n::ClapI18n;
use std::path::{Path, PathBuf};
use std::collections::{HashSet, HashMap};
use anni_common::validator::*;
use serde::{Deserialize, Deserializer};
use std::rc::Rc;
use std::iter::FromIterator;
use std::str::FromStr;
use serde::de::Error;
use crate::config::read_config;
use anni_flac::{FlacHeader, MetadataBlockData};
use anni_flac::blocks::{BlockVorbisComment, BlockStreamInfo, PictureType};

pub(crate) struct ConventionSubcommand;

impl Subcommand for ConventionSubcommand {
    fn name(&self) -> &'static str {
        "convention"
    }

    fn create(&self) -> App<'static> {
        App::new("convention")
            .about_ll("convention")
            .alias("conv")
            .subcommand(App::new("check")
                .about_ll("convention-check")
                .arg(Arg::new("fix")
                    .about_ll("convention-check-apply-fixed")
                    .long("fix")
                    .short('f')
                )
                .arg(Arg::new("Filename")
                    .takes_value(true)
                    .required(true)
                    .min_values(1)
                )
            )
    }

    fn handle(&self, matches: &ArgMatches) -> anyhow::Result<()> {
        // Initialize rules
        let config: ConventionConfig = read_config("convention").map_err(|e| {
            debug!(target: "convention", "Failed to read convention.toml: {}", e);
            debug!(target: "convention", "Using default anni convention");
            e
        }).unwrap_or_default();
        let rules = config.into_rules();

        if let Some(matches) = matches.subcommand_matches("check") {
            info!(target: "anni", "Convention validation started...");
            let fix = matches.is_present("apply-fixes");
            for input in matches.values_of_os("Filename").unwrap() {
                for (file, flac) in parse_input_iter(input) {
                    match flac {
                        Ok(mut flac) => {
                            rules.validate(file, &mut flac, fix);
                        }
                        Err(e) => error!(target: "convention/parse", "Failed to parse header of file {}: {:?}", file.to_string_lossy(), e),
                    }
                }
            }
            info!(target: "anni", "Convention validation finished.");
        }
        Ok(())
    }
}

struct ConventionRules {
    stream_info: ConventionStreamInfo,
    types: HashMap<String, Vec<Validator>>,

    required: HashMap<String, Rc<ConventionTag>>,
    optional: HashMap<String, Rc<ConventionTag>>,
    unrecommended: HashMap<String, Rc<ConventionTag>>,
}

impl ConventionRules {
    pub(crate) fn validate<P>(&self, filename: P, flac: &mut FlacHeader, fix: bool)
        where P: AsRef<Path> {
        let mut fixed = false;

        // validate stream info
        self.validate_stream_info(filename.as_ref(), flac.stream_info());

        // TODO: option to control whether cover validation should take effect
        // validate cover existence
        let mut has_cover = false;
        for block in flac.blocks.iter() {
            if let MetadataBlockData::Picture(data) = &block.data {
                if let PictureType::CoverFront = data.picture_type {
                    has_cover = true;
                }
            }
        }

        if !has_cover {
            error!(target: "convention/cover", "Cover does not exist in file {}!", filename.as_ref().to_string_lossy());
        }

        // validate comments
        match flac.comments() {
            None => error!(target: "convention/comment", "No VorbisComment block found in file {}!", filename.as_ref().to_string_lossy()),
            Some(_) => {
                let c = flac.comments_mut();
                let (comment_fixed, new_path) = self.validate_tags(filename.as_ref(), c, fix);
                fixed |= comment_fixed;

                // apply fixes
                if fixed {
                    flac.save::<String>(None).expect("Failed to save flac file");
                }
                if let Some(new_path) = new_path {
                    std::fs::rename(filename, new_path).unwrap();
                }
            }
        }
    }

    fn validate_stream_info<P>(&self, filename: P, info: &BlockStreamInfo)
        where P: AsRef<Path> {
        let filename = filename.as_ref().to_string_lossy();
        self.stream_info.sample_rate.map(|expected| {
            if info.sample_rate != expected {
                error!(target: "convention/sample-rate", "Stream sample-rate mismatch in file {}: expected {}, got {}", filename, expected, info.sample_rate);
            }
        });
        self.stream_info.bit_per_sample.map(|expected| {
            if info.bits_per_sample != expected {
                error!(target: "convention/bit-per-sample", "Stream bit-per-sample mismatch in file {}: expected {}, got {}", filename, expected, info.bits_per_sample);
            }
        });
        self.stream_info.channels.map(|expected| {
            if info.channels != expected {
                error!(target: "convention/channel-num", "Stream channel num mismatch in file {}: expected {}, got {}", filename, expected, info.channels);
            }
        });
    }

    fn validate_tags<P>(&self, filename: P, comment: &mut BlockVorbisComment, fix: bool) -> (bool, Option<PathBuf>)
        where P: AsRef<Path> {
        let mut fixed = false;
        let mut new_path = None;

        let filename_str = filename.as_ref().to_string_lossy();

        let mut required: HashSet<&str> = self.required.keys().map(|s| s.as_str()).collect();
        let (mut track_number, mut title) = (None, None);
        for comment in comment.comments.iter_mut() {
            let (key, key_raw, value) = (comment.key(), comment.key_raw(), comment.value());
            if value.is_empty() {
                warn!(target: "convention/tag/empty", "Empty value for tag: {} in file: {}", key_raw, filename_str);
            }
            if !comment.is_key_uppercase() {
                warn!(target: "convention/tag/lowercase", "Lowercase tag: {} in file: {}", key_raw, filename_str);
            }
            let key = key.as_str();

            let tag = if self.required.contains_key(key) {
                if !required.contains(key) {
                    // Required key duplicated
                    // duplication detection is only enabled for Required tags
                    warn!(target: "convention/tag/duplicated", "Required key {} duplicated in file: {}", key_raw, filename_str);
                    continue;
                } else {
                    // remove from required key set
                    // required tag
                    required.remove(key);
                    &self.required[key]
                }
            } else if self.optional.contains_key(key) {
                // optional tag
                &self.optional[key]
            } else if self.unrecommended.contains_key(key) {
                // unrecommended tag
                let tag = &self.unrecommended[key];
                warn!(target: "convention/tag/unrecommended", "Unrecommended key: {}={} found in file {}, use {} instead", key_raw, value, filename_str, &tag.name);
                tag
            } else {
                // No tag rule found
                warn!(target: "convention/tag/unnecessary", "Unnecessary tag {} in file: {}", key_raw, filename_str);
                if fix {
                    comment.clear();
                    fixed = true;
                }
                continue;
            };

            // type validators
            for v in self.types[tag.value_type.as_str()].iter() {
                if !v.validate(value) {
                    error!(target: "convention/tag/type", "Type validator {} not passed: invalid tag value {}={} in file: {}", v.name(), key_raw, value, filename_str);
                }
            }

            // field validators
            if let Err(v) = tag.validate(value) {
                error!(target: "convention/tag/validator", "Validator {} not passed: invalid tag value {}={} in file {}", v, key_raw, value, filename_str);
            } else if &tag.name == "TITLE" {
                // save track title for further use
                title = Some(value.to_string());
            } else if &tag.name == "TRACKNUMBER" {
                // save track number for further use
                track_number = Some(value.to_string());
            } else if &tag.name == "ARTIST" {
                // additional artist name check
                match value {
                    "[Unknown Artist]" => error!(target: "convention/tag/artist", "Invalid artist: {} in file: {}", value, filename_str),
                    "Various Artists" => warn!(target: "convention/tag/artist", "Various Artist is used as track artist in file: {}. Could it be more accurate?", filename_str),
                    _ => {}
                }
            }
        }

        // remaining keys in set are missing
        for key in required {
            error!(target: "convention/tag/missing", "Missing tag {} in file: {}", key, filename_str);
        }

        // Filename check
        if let (Some(title), Some(track_number)) = (title, track_number) {
            let filename_expected: &str = &format!("{:0>2}. {}.flac", track_number, title).replace("/", "／");
            let filename_raw = filename.as_ref().file_name().unwrap().to_str().expect("Non-UTF8 filenames are currently not supported!");
            if filename_raw != filename_expected {
                error!(target: "convention/filename", "Filename of file: {} mismatch. Expected {}", filename_str, filename_expected);
                if fix {
                    // use correct filename
                    let path_expected = filename.as_ref().with_file_name(filename_expected);
                    new_path = Some(path_expected);
                }
            }
        }

        // retain non-empty comments
        comment.comments.retain(|c| !c.is_empty());

        // return whether comment has been modified
        (fixed, new_path)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct ConventionConfig {
    #[serde(default)]
    stream_info: ConventionStreamInfo,
    types: HashMap<String, Vec<Validator>>,
    tags: ConventionTagConfig,
}

impl ConventionConfig {
    pub(crate) fn into_rules(self) -> ConventionRules {
        let mut rules = ConventionRules {
            stream_info: self.stream_info,
            types: self.types,
            required: Default::default(),
            optional: Default::default(),
            unrecommended: Default::default(),
        };
        for tag in self.tags.required {
            let this = Rc::new(tag);
            for key in this.alias.iter() {
                rules.unrecommended.insert(key.clone(), this.clone());
            }
            rules.required.insert(this.name.clone(), this);
        }

        for tag in self.tags.optional {
            let this = Rc::new(tag);
            for key in this.alias.iter() {
                rules.optional.insert(key.clone(), this.clone());
            }
            rules.optional.insert(this.name.clone(), this);
        }

        rules
    }
}

impl Default for ConventionConfig {
    fn default() -> Self {
        Self {
            stream_info: Default::default(),
            types: HashMap::from_iter(vec![
                ("string".to_string(), vec![Validator::from_str("trim").unwrap(), Validator::from_str("dot").unwrap()]),
                ("number".to_string(), vec![Validator::from_str("number").unwrap()])
            ].into_iter()),
            tags: ConventionTagConfig {
                required: vec![
                    ConventionTag {
                        name: "TITLE".to_string(),
                        alias: Default::default(),
                        value_type: ValueType::String,
                        validators: Default::default(),
                    },
                    ConventionTag {
                        name: "ARTIST".to_string(),
                        alias: Default::default(),
                        value_type: ValueType::String,
                        validators: vec![Validator::from_str("artist").unwrap()],
                    },
                    ConventionTag {
                        name: "ALBUM".to_string(),
                        alias: Default::default(),
                        value_type: ValueType::String,
                        validators: Default::default(),
                    },
                    ConventionTag {
                        name: "DATE".to_string(),
                        alias: Default::default(),
                        value_type: ValueType::String,
                        validators: vec![Validator::from_str("date").unwrap()],
                    },
                    ConventionTag {
                        name: "TRACKNUMBER".to_string(),
                        alias: Default::default(),
                        value_type: ValueType::Number,
                        validators: Default::default(),
                    },
                    ConventionTag {
                        name: "TRACKTOTAL".to_string(),
                        alias: HashSet::from_iter(vec!["TOTALTRACKS".to_string()].into_iter()),
                        value_type: ValueType::Number,
                        validators: Default::default(),
                    },
                ],
                optional: vec![
                    ConventionTag {
                        name: "DISCNUMBER".to_string(),
                        alias: Default::default(),
                        value_type: ValueType::Number,
                        validators: Default::default(),
                    },
                    ConventionTag {
                        name: "DISCTOTAL".to_string(),
                        alias: HashSet::from_iter(vec!["TOTALDISCS".to_string()].into_iter()),
                        value_type: ValueType::Number,
                        validators: Default::default(),
                    },
                    ConventionTag {
                        name: "ALBUMARTIST".to_string(),
                        alias: Default::default(),
                        value_type: ValueType::String,
                        validators: Default::default(),
                    }
                ],
            },
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct ConventionStreamInfo {
    sample_rate: Option<u32>,
    channels: Option<u8>,
    bit_per_sample: Option<u8>,
}

impl Default for ConventionStreamInfo {
    fn default() -> Self {
        Self {
            sample_rate: Some(44100),
            channels: Some(2),
            bit_per_sample: Some(16),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct ConventionTagConfig {
    required: Vec<ConventionTag>,
    optional: Vec<ConventionTag>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct ConventionTag {
    /// Tag name
    name: String,

    /// A Set of name aliases
    #[serde(default)]
    alias: HashSet<String>,

    /// Value inner type
    #[serde(rename = "type")]
    value_type: ValueType,

    /// Validators for this tag
    #[serde(default)]
    validators: Vec<Validator>,
}

impl ConventionTag {
    pub(crate) fn validate(&self, value: &str) -> Result<(), &'static str> {
        for v in self.validators.iter() {
            if !v.validate(value) {
                return Err(v.name());
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
enum ValueType {
    String,
    Number,
}

impl ValueType {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            ValueType::String => "string",
            ValueType::Number => "number"
        }
    }
}

impl<'de> Deserialize<'de> for ValueType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'de>
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "string" => Ok(ValueType::String),
            "number" => Ok(ValueType::Number),
            _ => Err(D::Error::custom("invalid ValueType")),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::subcommands::convention::ConventionConfig;

    #[test]
    fn test_default_config() {
        let result = ConventionConfig::default();
        println!("{:#?}", result);
    }
}