use std::fmt::{Display, Formatter};
use clap::{Args, ArgEnum};
use anni_repo::RepositoryManager;
use anni_clap_handler::handler;
use anni_common::validator::{ValidateResult, ValidatorList};
use anni_repo::prelude::*;
use crate::{fl, ball};
use serde::Serialize;
use anni_common::diagnostic::*;
use anni_common::lint::{AnniLinter, AnniLinterReviewDogJsonLineFormat, AnniLinterTextFormat};

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
    #[clap(name = "rdjsonl")]
    ReviewDogJsonLines,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "lowercase", tag = "type")]
enum RepoLintTarget {
    Album { album_id: String },
    Disc { album_id: String, disc_id: u8 },
    Track { album_id: String, disc_id: u8, track_id: u8 },
}

impl Display for RepoLintTarget {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RepoLintTarget::Album { album_id } => write!(f, "{album_id}"),
            RepoLintTarget::Disc { album_id, disc_id } => write!(f, "{album_id}/{disc_id}"),
            RepoLintTarget::Track { album_id, disc_id, track_id } => write!(f, "{album_id}/{disc_id}/{track_id}"),
        }
    }
}

#[handler(RepoLintAction)]
fn repo_lint(manager: RepositoryManager, me: &RepoLintAction) -> anyhow::Result<()> {
    info!(target: "anni", "{}", fl!("repo-validate-start"));

    let mut report: Box<dyn AnniLinter> = match me.format {
        RepoLintFormat::Text => Box::new(AnniLinterTextFormat::default()),
        RepoLintFormat::ReviewDogJsonLines => Box::new(AnniLinterReviewDogJsonLineFormat::default()),
    };

    if me.albums.is_empty() {
        // initialize owned manager
        let manager = manager.into_owned_manager()?;
        // validate all albums
        for album in manager.albums_iter() {
            validate_album(album, report.as_mut());
        }
        // check tag loop
        if let Some(path) = manager.check_tags_loop() {
            report.add(Diagnostic {
                message: format!("Loop detected: {:?}", path),
                location: Default::default(),
                severity: DiagnosticSeverity::Error,
                source: Some(DiagnosticSource {
                    name: "tags".to_string(),
                    url: None,
                }),
                ..Default::default()
            });
        }
    } else {
        // validate selected albums
        for album in me.albums.iter() {
            for album in manager.load_albums(album)? {
                validate_album(&album, report.as_mut());
            }
        }
    }

    if !report.flush() {
        ball!("repo-validate-failed");
    }

    info!(target: "anni", "{}", fl!("repo-validate-end"));
    Ok(())
}

fn validate_album(album: &Album, report: &mut dyn AnniLinter) {
    let album_id = album.album_id().to_string();

    let string_validator = ValidatorList::new(&["trim", "dot", "tidle"]).unwrap();
    let artist_validator = ValidatorList::new(&["trim", "dot", "tidle", "artist"]).unwrap();

    validate_string(RepoLintTarget::Album { album_id: album_id.clone() }, Some("title".to_string()), &string_validator, album.title().as_ref(), report);
    validate_string(RepoLintTarget::Album { album_id: album_id.clone() }, Some("artist".to_string()), &artist_validator, album.artist(), report);

    if album.artist() == "[Unknown Artist]" || album.artist() == "UnknownArtist" {
        report.add(Diagnostic {
            message: "Unknown artist".to_string(),
            severity: DiagnosticSeverity::Error,
            code: Some(DiagnosticCode::new(album_id.clone())),
            ..Default::default()
        });
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

fn validate_string(target: RepoLintTarget, field: Option<String>, validator: &ValidatorList, value: &str, report: &mut dyn AnniLinter) {
    validator.validate(value).into_iter().for_each(|(ty, result)| {
        let severity = match result {
            ValidateResult::Warning(_) => DiagnosticSeverity::Warning,
            ValidateResult::Error(_) => DiagnosticSeverity::Error,
            _ => DiagnosticSeverity::Info,
        };
        match result {
            ValidateResult::Warning(message) | ValidateResult::Error(message) => {
                report.add(Diagnostic {
                    severity,
                    message,
                    location: DiagnosticLocation::simple(target.to_string()),
                    code: Some(DiagnosticCode::new(format!("{}|{}", ty, field.as_deref().unwrap_or_default()))),
                    ..Default::default()
                });
            }
            _ => {}
        }
    });
}
