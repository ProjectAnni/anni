use std::fmt::{Display, Formatter};
use clap::{Args, ArgEnum};
use anni_repo::RepositoryManager;
use anni_clap_handler::handler;
use anni_common::validator::{ValidateResult, ValidatorList};
use anni_repo::prelude::*;
use crate::{fl, ball};
use serde::Serialize;

#[derive(Args, Debug, Clone)]
pub struct RepoLintAction {
    #[clap(short, long)]
    #[clap(arg_enum, default_value = "text")]
    format: RepoLintFormat,

    albums: Vec<String>,
}

#[derive(ArgEnum, Clone, Debug)]
pub enum RepoLintFormat {
    Text,
    // Markdown,
    Json,
}

#[derive(Default, Serialize)]
struct RepoLintResult {
    errors: Vec<RepoLintItem>,
    warnings: Vec<RepoLintItem>,
}

#[derive(Serialize)]
struct RepoLintItem {
    target: RepoLintTarget,
    field: Option<String>,
    initiator: Option<String>,
    message: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "lowercase", tag = "type")]
enum RepoLintTarget {
    Album { album_id: String },
    Disc { album_id: String, disc_id: u8 },
    Track { album_id: String, disc_id: u8, track_id: u8 },
    Any(String),
}

impl RepoLintResult {
    fn add_error(&mut self, target: RepoLintTarget, field: Option<String>, initiator: Option<String>, message: String) {
        self.errors.push(RepoLintItem { target, field, initiator, message });
    }

    fn add_warning(&mut self, target: RepoLintTarget, field: Option<String>, initiator: Option<String>, message: String) {
        self.warnings.push(RepoLintItem { target, field, initiator, message });
    }

    fn has_error(&self) -> bool {
        !self.errors.is_empty()
    }

    fn is_empty(&self) -> bool {
        self.errors.is_empty() && self.warnings.is_empty()
    }
}

impl RepoLintItem {
    fn target(&self) -> String {
        match &self.field {
            None => format!("{}", self.target),
            Some(field) => format!("{}.{}", self.target, field),
        }
    }
}

impl Display for RepoLintTarget {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RepoLintTarget::Album { album_id } => write!(f, "{album_id}"),
            RepoLintTarget::Disc { album_id, disc_id } => write!(f, "{album_id}/{disc_id}"),
            RepoLintTarget::Track { album_id, disc_id, track_id } => write!(f, "{album_id}/{disc_id}/{track_id}"),
            RepoLintTarget::Any(target) => write!(f, "{target}"),
        }
    }
}

#[handler(RepoLintAction)]
fn repo_lint(manager: RepositoryManager, me: &RepoLintAction) -> anyhow::Result<()> {
    info!(target: "anni", "{}", fl!("repo-validate-start"));

    let mut report = RepoLintResult::default();

    if me.albums.is_empty() {
        // initialize owned manager
        let manager = manager.into_owned_manager()?;
        // validate all albums
        for album in manager.albums_iter() {
            validate_album(album, &mut report);
        }
        // check tag loop
        if let Some(path) = manager.check_tags_loop() {
            report.add_error(RepoLintTarget::Any("tags".to_string()), None, None, format!("Loop detected: {:?}", path));
        }
    } else {
        // validate selected albums
        for album in me.albums.iter() {
            for album in manager.load_albums(album)? {
                validate_album(&album, &mut report);
            }
        }
    }

    match me.format {
        RepoLintFormat::Text => {
            println!("{} errors, {} warnings", report.errors.len(), report.warnings.len());
            if !report.is_empty() {
                println!();
                for error in report.errors.iter() {
                    println!("[ERROR][{}] {}", error.target(), error.message);
                }
                println!();
                for warning in report.warnings.iter() {
                    println!("[WARN][{}] {}", warning.target(), warning.message);
                }
            }
        }
        // RepoLintFormat::Markdown => {}
        RepoLintFormat::Json => println!("{}", serde_json::to_string(&report)?),
    }


    if !report.has_error() {
        info!(target: "anni", "{}", fl!("repo-validate-end"));
        Ok(())
    } else {
        ball!("repo-validate-failed");
    }
}

fn validate_album(album: &Album, report: &mut RepoLintResult) {
    let album_id = album.album_id().to_string();

    let string_validator = ValidatorList::new(&["trim", "dot", "tidle"]).unwrap();
    let artist_validator = ValidatorList::new(&["trim", "dot", "tidle", "artist"]).unwrap();

    validate_string(RepoLintTarget::Album { album_id: album_id.clone() }, Some("title".to_string()), &string_validator, album.title().as_ref(), report);
    validate_string(RepoLintTarget::Album { album_id: album_id.clone() }, Some("artist".to_string()), &artist_validator, album.artist(), report);

    if album.artist() == "[Unknown Artist]" || album.artist() == "UnknownArtist" {
        report.add_error(RepoLintTarget::Album { album_id: album_id.clone() }, Some("artist".to_string()), None, "Unknown artist".to_string());
    }

    for (disc_id, disc) in album.discs().iter().enumerate() {
        let disc_id = (disc_id + 1) as u8;

        validate_string(RepoLintTarget::Disc { album_id: album_id.clone(), disc_id }, Some("title".to_string()), &string_validator, disc.title(), report);
        validate_string(RepoLintTarget::Disc { album_id: album_id.clone(), disc_id }, Some("artist".to_string()), &artist_validator, disc.artist(), report);

        for (track_id, track) in disc.tracks().iter().enumerate() {
            let track_id = (track_id + 1) as u8;

            validate_string(RepoLintTarget::Track { album_id: album_id.clone(), disc_id, track_id }, Some("title".to_string()), &string_validator, track.title().as_ref(), report);
            validate_string(RepoLintTarget::Track { album_id: album_id.clone(), disc_id, track_id }, Some("artist".to_string()), &artist_validator, track.artist(), report);
        }
    }
}

fn validate_string(target: RepoLintTarget, field: Option<String>, validator: &ValidatorList, value: &str, report: &mut RepoLintResult) {
    validator.validate(value).into_iter().for_each(|(ty, result)| {
        match result {
            ValidateResult::Warning(error) => {
                report.add_warning(target.clone(), field.clone(), Some(ty.to_string()), error);
            }
            ValidateResult::Error(error) => {
                report.add_error(target.clone(), field.clone(), Some(ty.to_string()), error);
            }
            _ => {}
        }
    });
}
