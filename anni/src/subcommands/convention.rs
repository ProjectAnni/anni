use crate::args::{FlacInputPath, InputPath};
use crate::config::read_config;
use crate::ll;
use clap_handler::{handler, Context, Handler};
use anni_common::validator::*;
use anni_flac::blocks::{BlockStreamInfo, BlockVorbisComment, PictureType};
use anni_flac::{FlacHeader, MetadataBlockData};
use clap::{Args, Subcommand};
use serde::de::Error;
use serde::{Deserialize, Deserializer};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Args, Debug, Clone, Handler)]
#[clap(about = ll ! ("convention"))]
#[clap(alias = "conv")]
#[handler_inject(convention_rules)]
pub struct ConventionSubcommand {
    #[clap(subcommand)]
    action: ConventionAction,
}

impl ConventionSubcommand {
    async fn convention_rules(&self, ctx: &mut Context) -> anyhow::Result<()> {
        // Initialize rules
        let config: ConventionConfig = read_config("convention")
            .map_err(|e| {
                debug!(target: "convention", "Failed to read convention.toml: {}", e);
                debug!(target: "convention", "Using default anni convention");
                e
            })
            .unwrap_or_default();
        let rules = config.into_rules();
        ctx.insert(rules);
        Ok(())
    }
}

#[derive(Subcommand, Handler, Debug, Clone)]
pub enum ConventionAction {
    #[clap(about = ll ! ("convention-check"))]
    Check(ConventionCheckAction),
}

#[derive(Args, Debug, Clone)]
pub struct ConventionCheckAction {
    #[clap(short, long)]
    #[clap(help = ll ! ("convention-check-fix"))]
    fix: bool,

    #[clap(required = true)]
    filename: Vec<InputPath<FlacInputPath>>,
}

#[handler(ConventionCheckAction)]
fn convention_check(me: &ConventionCheckAction, rules: &ConventionRules) -> anyhow::Result<()> {
    info!(target: "anni", "Convention validation started...");
    for input in &me.filename {
        for file in input.iter() {
            let flac = FlacHeader::from_file(file.as_path());
            match flac {
                Ok(mut flac) => {
                    rules.validate(file, &mut flac, me.fix);
                }
                Err(e) => {
                    error!(target: "convention/parse", "Failed to parse header of file {}: {:?}", file.to_string_lossy(), e)
                }
            }
        }
    }
    info!(target: "anni", "Convention validation finished.");
    Ok(())
}

struct ConventionRules {
    stream_info: ConventionStreamInfo,
    types: HashMap<String, ValidatorList>,

    required: HashMap<String, Arc<ConventionTag>>,
    optional: HashMap<String, Arc<ConventionTag>>,
    unrecommended: HashMap<String, Arc<ConventionTag>>,
}

impl ConventionRules {
    pub(crate) fn validate<P>(&self, filename: P, flac: &mut FlacHeader, fix: bool)
        where
            P: AsRef<Path>,
    {
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
            None => {
                error!(target: "convention/comment", "No VorbisComment block found in file {}!", filename.as_ref().to_string_lossy())
            }
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
        where
            P: AsRef<Path>,
    {
        let filename = filename.as_ref().to_string_lossy();
        self.stream_info.sample_rate.iter().for_each(|expected| {
            if !expected.contains(&info.sample_rate) {
                error!(target: "convention/sample-rate", "Stream sample-rate mismatch in file {filename}: expected `{expected:?}`, got {}", info.sample_rate);
            }
        });
        self.stream_info.bit_per_sample.iter().for_each(|expected| {
            if info.bits_per_sample != *expected {
                error!(target: "convention/bit-per-sample", "Stream bit-per-sample mismatch in file {filename}: expected {expected}, got {}", info.bits_per_sample);
            }
        });
        self.stream_info.channels.iter().for_each(|expected| {
            if info.channels != *expected {
                error!(target: "convention/channel-num", "Stream channel num mismatch in file {filename}: expected {expected}, got {}", info.channels);
            }
        });
        if self.stream_info.require_checksum && u128::from_be_bytes(info.md5_signature) == 0 {
            error!(target: "convention/checksum", "Empty checksum detected in file: {filename}");
        }
    }

    fn validate_tags<P>(
        &self,
        filename: P,
        comment: &mut BlockVorbisComment,
        fix: bool,
    ) -> (bool, Option<PathBuf>)
        where
            P: AsRef<Path>,
    {
        let mut fixed = false;
        let mut new_path = None;

        let filename_str = filename.as_ref().to_string_lossy();

        let mut required: HashSet<&str> = self.required.keys().map(|s| s.as_str()).collect();
        let (mut track_number, mut title) = (None, None);
        for comment in comment.comments.iter_mut() {
            let (key, key_raw, value) = (comment.key(), comment.key_raw(), comment.value());
            if value.is_empty() {
                warn!(target: "convention/tag/empty", "Empty value for tag: {key_raw} in file: {filename_str}");
            }
            if !comment.is_key_uppercase() {
                warn!(target: "convention/tag/lowercase", "Lowercase tag: {key_raw} in file: {filename_str}");
            }
            let key = key.as_str();

            let tag = if self.required.contains_key(key) {
                if !required.contains(key) {
                    // Required key duplicated
                    // duplication detection is only enabled for Required tags
                    warn!(target: "convention/tag/duplicated", "Required key {key_raw} duplicated in file: {filename_str}");
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
                warn!(target: "convention/tag/unrecommended", "Unrecommended key: {key_raw}={value} found in file {filename_str}, use {} instead", &tag.name);
                tag
            } else {
                // No tag rule found
                warn!(target: "convention/tag/unnecessary", "Unnecessary tag {key_raw} in file: {filename_str}");
                if fix {
                    comment.clear();
                    fixed = true;
                }
                continue;
            };

            // type validators
            for (ty, result) in self.types[tag.value_type.as_str()].validate(value) {
                // if it's a warning, display it with warn!
                match result {
                    ValidateResult::Warning(message) => {
                        warn!(target: "convention/tag/type", "Validator {ty} warning for tag {key_raw}={value} in file {filename_str}: {message}");
                    }
                    ValidateResult::Error(message) => {
                        error!(target: "convention/tag/type", "Type validator {ty} not passed: invalid tag value {key_raw}={value} in file {filename_str}: {message}");
                    }
                    _ => {}
                }
            }

            // field validators
            for (ty, result) in tag.validate(value) {
                match result {
                    ValidateResult::Warning(message) => {
                        warn!(target: "convention/tag/validator", "Validator {ty} warning for tag {key_raw}={value} in file {filename_str}: {message}");
                    }
                    ValidateResult::Error(message) => {
                        error!(target: "convention/tag/validator", "Validator {ty} not passed: invalid tag value {key_raw}={value} in file {filename_str}: {message}");
                    }
                    _ => {}
                }
            }

            if &tag.name == "TITLE" {
                // save track title for further use
                title = Some(value.to_string());
            } else if &tag.name == "TRACKNUMBER" {
                // save track number for further use
                track_number = Some(value.to_string());
            } else if &tag.name == "ARTIST" {
                // additional artist name check
                match value {
                    "[Unknown Artist]" | "UnknownArtist" => {
                        error!(target: "convention/tag/artist", "Invalid artist: {value} in file: {filename_str}")
                    }
                    "Various Artists" => {
                        warn!(target: "convention/tag/artist", "Various Artist is used as track artist in file: {filename_str}. Could it be more accurate?")
                    }
                    _ => {}
                }
            }
        }

        // remaining keys in set are missing
        for key in required {
            error!(target: "convention/tag/missing", "Missing tag {key} in file: {filename_str}");
        }

        // Filename check
        if let (Some(title), Some(track_number)) = (title, track_number) {
            let filename_expected: &str =
                &format!("{:0>2}. {}.flac", track_number, title).replace("/", "／");
            let filename_raw = filename
                .as_ref()
                .file_name()
                .unwrap()
                .to_str()
                .expect("Non-UTF8 filenames are currently not supported!");
            if filename_raw != filename_expected {
                error!(target: "convention/filename", "Filename of file: {filename_str} mismatch. Expected {filename_expected}");
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
    types: HashMap<String, ValidatorList>,
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
            let this = Arc::new(tag);
            for key in this.alias.iter() {
                rules.unrecommended.insert(key.clone(), this.clone());
            }
            rules.required.insert(this.name.clone(), this);
        }

        for tag in self.tags.optional {
            let this = Arc::new(tag);
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
            types: vec![
                ("string".to_string(), ValidatorList::new(&["trim", "dot", "tidle"]).unwrap()),
                ("number".to_string(), ValidatorList::new(&["number"]).unwrap()),
            ]
                .into_iter()
                .collect(),
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
                        validators: ValidatorList::new(&["artist"]).unwrap(),
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
                        validators: ValidatorList::new(&["date"]).unwrap(),
                    },
                    ConventionTag {
                        name: "TRACKNUMBER".to_string(),
                        alias: Default::default(),
                        value_type: ValueType::Number,
                        validators: Default::default(),
                    },
                    ConventionTag {
                        name: "TRACKTOTAL".to_string(),
                        alias: vec!["TOTALTRACKS".to_string()].into_iter().collect(),
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
                        alias: vec!["TOTALDISCS".to_string()].into_iter().collect(),
                        value_type: ValueType::Number,
                        validators: Default::default(),
                    },
                    ConventionTag {
                        name: "ALBUMARTIST".to_string(),
                        alias: Default::default(),
                        value_type: ValueType::String,
                        validators: Default::default(),
                    },
                ],
            },
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct ConventionStreamInfo {
    sample_rate: Option<Vec<u32>>,
    channels: Option<u8>,
    bit_per_sample: Option<u8>,
    require_checksum: bool,
}

impl Default for ConventionStreamInfo {
    fn default() -> Self {
        Self {
            sample_rate: Some(vec![44100, 48000]),
            channels: Some(2),
            bit_per_sample: Some(16),
            require_checksum: true,
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
    validators: ValidatorList,
}

impl ConventionTag {
    pub(crate) fn validate(&self, value: &str) -> Vec<(&'static str, ValidateResult)> {
        self.validators.validate(value)
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
            ValueType::Number => "number",
        }
    }
}

impl<'de> Deserialize<'de> for ValueType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
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
