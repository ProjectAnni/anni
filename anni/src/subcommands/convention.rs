use clap::{ArgMatches, App, Arg};
use crate::subcommands::Subcommand;
use crate::subcommands::flac::parse_input_iter;
use crate::i18n::ClapI18n;
use std::path::Path;
use shell_escape::escape;
use std::collections::{HashSet, HashMap};
use anni_common::validator::*;
use serde::{Deserialize, Deserializer};
use std::rc::Rc;
use std::iter::FromIterator;
use anni_flac::blocks::{BlockVorbisComment, BlockStreamInfo};
use std::str::FromStr;
use serde::de::Error;
use crate::config::read_config;
use anni_flac::FlacHeader;

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
            for input in matches.values_of_os("Filename").unwrap() {
                for (file, header) in parse_input_iter(input) {
                    match header {
                        Ok(header) => {
                            rules.validate(file, &header);
                        }
                        Err(e) => error!(target: &format!("convention][{}", file.to_string_lossy()), "Failed to parse header: {:?}", e),
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
    pub(crate) fn validate<P>(&self, filename: P, flac: &FlacHeader)
        where P: AsRef<Path> {
        self.validate_stream_info(filename.as_ref(), flac.stream_info());

        // validate comments
        let fixes = flac.comments()
            .map_or_else(|| {
                error!(target: &format!("convention][{}", filename.as_ref().to_string_lossy()), "No VorbisComment block found!");
                Vec::new()
            }, |c| self.validate_tags(filename.as_ref(), c));

        for fix in fixes {
            println!("{}", fix);
        }
    }

    fn validate_stream_info<P>(&self, filename: P, info: &BlockStreamInfo)
        where P: AsRef<Path> {
        let filename = filename.as_ref().to_string_lossy();
        self.stream_info.sample_rate.map(|expected| {
            if info.sample_rate != expected {
                error!(target: &format!("convention][{}", filename), "Stream sample-rate mismatch: expected {}, got {}", expected, info.sample_rate);
            }
        });
        self.stream_info.bit_per_sample.map(|expected| {
            if info.bits_per_sample != expected {
                error!(target: &format!("convention][{}", filename), "Stream bit-per-sample mismatch: expected {}, got {}", expected, info.bits_per_sample);
            }
        });
        self.stream_info.channels.map(|expected| {
            if info.channels != expected {
                error!(target: &format!("convention][{}", filename), "Stream channel num mismatch: expected {}, got {}", expected, info.channels);
            }
        });
    }

    fn validate_tags<P>(&self, filename: P, comment: &BlockVorbisComment) -> Vec<String>
        where P: AsRef<Path> {
        let filename_str = filename.as_ref().to_string_lossy();

        let mut fixes = Vec::default();
        let mut required: HashSet<&str> = self.required.keys().map(|s| s.as_str()).collect();
        let (mut track_number, mut title) = (None, None);
        for comment in comment.comments.iter() {
            let (key, key_raw, value) = (comment.key(), comment.key_raw(), comment.value());
            if value.is_empty() {
                warn!(target: &format!("convention][{}", filename_str), "Empty value for tag: {}", key_raw);
            }
            if !comment.is_key_uppercase() {
                warn!(target: &format!("convention][{}", filename_str), "Lowercase tag: {}", key_raw);
            }
            let key = key.as_str();

            let tag = if self.required.contains_key(key) {
                if !required.contains(key) {
                    // Required key duplicated
                    // duplication detection is only enabled for Required tags
                    warn!(target: &format!("convention][{}", filename_str), "Required key duplicated: {}", key_raw);
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
                warn!(target: &format!("convention][{}", filename_str), "Unrecommended key: {}={}, use {} instead", key_raw, value, &tag.name);
                tag
            } else {
                // No tag rule found
                warn!(target: &format!("convention][{}", filename_str), "Unnecessary tag: {}", key_raw);
                fixes.push(format!("metaflac --remove-tag={} {}", escape(key_raw.into()), escape(filename.as_ref().to_string_lossy())));
                continue;
            };

            // type validators
            for v in self.types[tag.value_type.as_str()].iter() {
                if !v.validate(value) {
                    error!(target: &format!("convention][{}", filename_str), "Type validator {} not passed: invalid tag value {}={}", v.name(), key_raw, value);
                }
            }

            // field validators
            if let Err(v) = tag.validate(value) {
                error!(target: &format!("convention][{}", filename_str), "Validator {} not passed: invalid tag value {}={}", v, key_raw, value);
            } else if &tag.name == "TITLE" {
                title = Some(value.to_string());
            } else if &tag.name == "TRACKNUMBER" {
                track_number = Some(value.to_string());
            }
        }

        // remaining keys in set are missing
        for key in required {
            error!(target: &format!("convention][{}", filename_str), "Missing tag: {}", key);
        }

        // Filename check
        if let (Some(title), Some(track_number)) = (title, track_number) {
            let filename_expected: &str = &format!("{:0>2}. {}.flac", track_number, title).replace("/", "／");
            let filename_raw = filename.as_ref().file_name().unwrap().to_str().expect("Non-UTF8 filenames are currently not supported!");
            if filename_raw != filename_expected {
                error!(target: &format!("convention][{}", filename_str), "Filename mismatch. Expected {}", filename_expected);
                let path_expected = filename.as_ref().with_file_name(filename_expected);
                fixes.push(format!("mv {} {}", escape(filename.as_ref().to_string_lossy()), escape(path_expected.to_string_lossy())));
            }
        }
        fixes
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