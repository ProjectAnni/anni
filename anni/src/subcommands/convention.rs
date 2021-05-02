use clap::{ArgMatches, App, Arg};
use crate::subcommands::Subcommand;
use crate::subcommands::flac::parse_input_iter;
use crate::i18n::ClapI18n;
use std::path::Path;
use shell_escape::escape;
use std::collections::{HashSet, HashMap};
use anni_utils::validator::*;
use serde::Deserialize;
use std::rc::Rc;
use std::iter::FromIterator;
use anni_flac::blocks::BlockVorbisComment;
use std::str::FromStr;

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
        if let Some(matches) = matches.subcommand_matches("check") {
            for input in matches.values_of_os("Filename").unwrap().collect::<Vec<_>>() {
                for (file, header) in parse_input_iter(input) {
                    match header {
                        Ok(header) => {
                            let comments = header.comments().expect("Failed to read comments");
                            // TODO: user-defined rules
                            let rules = ConventionConfig::default().into_rules();
                            rules.validate(file, comments);
                        }
                        Err(e) => error!("Failed to parse header of {:?}: {:?}", file, e),
                    }
                }
            }
        }
        Ok(())
    }
}

struct ConventionRules {
    required: HashMap<String, Rc<ConventionTag>>,
    optional: HashMap<String, Rc<ConventionTag>>,
    unrecommended: HashMap<String, Rc<ConventionTag>>,
}

impl ConventionRules {
    pub(crate) fn validate<P>(&self, filename: P, comment: &BlockVorbisComment)
        where P: AsRef<Path> {
        info!("Checking {:?}", filename.as_ref());

        let mut fixes = Vec::default();
        let mut required: HashSet<&str> = self.required.keys().map(|s| s.as_str()).collect();
        let (mut track_number, mut title) = (None, None);
        for comment in comment.comments.iter() {
            let (key, value) = (comment.key_raw(), comment.value());
            if value.is_empty() {
                warn!("Empty value for tag: {}", key);
            }
            if !comment.is_key_uppercase() {
                warn!("Lowercase tag: {}", key);
            }

            let tag = if !required.contains(key) {
                // Required key duplicated
                // duplication detection is only enabled for Required tags
                warn!("Required key duplicated: {}", key);
                continue;
            } else if self.required.contains_key(key) {
                // remove from required key set
                // required tag
                required.remove(key);
                &self.required[key]
            } else if self.optional.contains_key(key) {
                // optional tag
                &self.optional[key]
            } else if self.unrecommended.contains_key(key) {
                // unrecommended tag
                let tag = &self.unrecommended[key];
                warn!("Unrecommended key: {}={}, use {} instead", key, value, &tag.name);
                tag
            } else {
                // No tag rule found
                warn!("Unnecessary tag: {}", key);
                fixes.push(format!("metaflac --remove-tag={} {}", escape(key.into()), escape(filename.as_ref().to_string_lossy())));
                continue;
            };

            if let Err(v) = tag.validate(value) {
                error!("Validator {} not passed: invalid tag value {}={}", v, key, value);
            } else if &tag.name == "TITLE" {
                title = Some(value.to_string());
            } else if &tag.name == "TRACKNUMBER" {
                track_number = Some(value.to_string());
            }
        }

        // remaining keys in set are missing
        for key in required {
            error!("Missing tag: {}", key);
        }

        // Filename check
        if let (Some(title), Some(track_number)) = (title, track_number) {
            let filename_expected: &str = &format!("{:02}. {}.flac", track_number, title).replace("/", "／");
            let filename_raw = filename.as_ref().file_name().unwrap().to_str().expect("Non-UTF8 filenames are currently not supported!");
            if filename_raw != filename_expected {
                error!("Filename mismatch, got {}, expected {}", filename_raw, filename_expected);
                let path_expected = filename.as_ref().with_file_name(filename_expected);
                fixes.push(format!("mv {} {}", escape(filename.as_ref().to_string_lossy()), escape(path_expected.to_string_lossy())));
            }
        }

        for fix in fixes.iter() {
            println!("{}", fix);
        }
    }
}

#[derive(Debug, Deserialize)]
struct ConventionConfig {
    tags: Vec<ConventionTag>,
}

impl ConventionConfig {
    pub(crate) fn into_rules(self) -> ConventionRules {
        let mut rules = ConventionRules { required: Default::default(), optional: Default::default(), unrecommended: Default::default() };

        for tag in self.tags {
            let this = Rc::new(tag);
            for key in this.alias.iter() {
                if this.required {
                    rules.unrecommended.insert(key.clone(), this.clone());
                } else {
                    rules.optional.insert(key.clone(), this.clone());
                }
            }
            if this.required {
                rules.required.insert(this.name.clone(), this);
            } else {
                rules.optional.insert(this.name.clone(), this);
            }
        }

        rules
    }
}

impl Default for ConventionConfig {
    fn default() -> Self {
        Self {
            tags: vec![
                ConventionTag {
                    name: "TITLE".to_string(),
                    alias: Default::default(),
                    required: true,
                    validators: vec![Validator::from_str("trim").unwrap()],
                },
                ConventionTag {
                    name: "ARTIST".to_string(),
                    alias: Default::default(),
                    required: true,
                    validators: vec![Validator::from_str("artist").unwrap()],
                },
                ConventionTag {
                    name: "ALBUM".to_string(),
                    alias: Default::default(),
                    required: true,
                    validators: vec![Validator::from_str("trim").unwrap()],
                },
                ConventionTag {
                    name: "DATE".to_string(),
                    alias: Default::default(),
                    required: true,
                    validators: vec![Validator::from_str("date").unwrap()],
                },
                ConventionTag {
                    name: "TRACKNUMBER".to_string(),
                    alias: Default::default(),
                    required: true,
                    validators: vec![Validator::from_str("number").unwrap()],
                },
                ConventionTag {
                    name: "TRACKTOTAL".to_string(),
                    alias: HashSet::from_iter(vec!["TOTALTRACKS".to_string()].into_iter()),
                    required: true,
                    validators: vec![Validator::from_str("number").unwrap()],
                },
                ConventionTag {
                    name: "DISCNUMBER".to_string(),
                    alias: Default::default(),
                    required: true,
                    validators: vec![Validator::from_str("number").unwrap()],
                },
                ConventionTag {
                    name: "DISCTOTAL".to_string(),
                    alias: HashSet::from_iter(vec!["TOTALDISCS".to_string()].into_iter()),
                    required: true,
                    validators: vec![Validator::from_str("number").unwrap()],
                },
                ConventionTag {
                    name: "ALBUMARTIST".to_string(),
                    alias: Default::default(),
                    required: true,
                    validators: vec![Validator::from_str("trim").unwrap()],
                }
            ]
        }
    }
}

#[derive(Debug, Deserialize)]
struct ConventionTag {
    /// Tag name
    name: String,

    /// A Set of name aliases
    #[serde(default)]
    alias: HashSet<String>,

    /// Whether this tag is required
    #[serde(default)]
    required: bool,

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

#[cfg(test)]
mod test {
    use crate::subcommands::convention::ConventionConfig;

    #[test]
    fn test_default_config() {
        let result = ConventionConfig::default();
        println!("{:#?}", result);
    }
}