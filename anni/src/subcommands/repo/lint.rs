use std::path::PathBuf;
use clap::{Args, ArgEnum};
use anni_repo::RepositoryManager;
use anni_clap_handler::handler;
use anni_common::validator::{ValidateResult, ValidatorList};
use anni_repo::prelude::*;
use crate::{fl, ball};
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

#[handler(RepoLintAction)]
fn repo_lint(manager: RepositoryManager, me: &RepoLintAction) -> anyhow::Result<()> {
    info!(target: "anni", "{}", fl!("repo-validate-start"));

    let mut report: Box<dyn AnniLinter<MetadataDiagnosticTarget>> = match me.format {
        RepoLintFormat::Text => Box::new(AnniLinterTextFormat::new()),
        RepoLintFormat::ReviewDogJsonLines => Box::new(AnniLinterReviewDogJsonLineFormat::new()),
    };

    if me.albums.is_empty() {
        // initialize owned manager
        let manager = manager.into_owned_manager()?;
        // validate all albums
        for album in manager.albums_iter() {
            let album_path = manager.album_path(&album.album_id().to_string()).unwrap();
            validate_album(album, album_path, report.as_mut());
        }
        // check tag loop
        if let Some(path) = manager.check_tags_loop() {
            report.add(Diagnostic::error(
                DiagnosticMessage {
                    message: format!("Tag loop relation detected: {:?}", path),
                    target: MetadataDiagnosticTarget::Tag(path[0].to_string()),
                },
                DiagnosticLocation::simple(manager.tag_path(&path[0]).unwrap().display().to_string()),
            ));
        }
    } else {
        // validate selected albums
        for album in me.albums.iter() {
            // FIXME: this may be incorrect
            for (album, path) in manager.load_albums(album)?.iter().zip(manager.album_paths(album)?) {
                validate_album(&album, &path, report.as_mut());
            }
        }
    }

    if !report.flush() {
        ball!("repo-validate-failed");
    }

    info!(target: "anni", "{}", fl!("repo-validate-end"));
    Ok(())
}

fn validate_album(album: &Album, path: &PathBuf, report: &mut dyn AnniLinter<MetadataDiagnosticTarget>) {
    let album_id = album.album_id().to_string();

    let string_validator = ValidatorList::new(&["trim", "dot", "tidle"]).unwrap();
    let artist_validator = ValidatorList::new(&["trim", "dot", "tidle", "artist"]).unwrap();

    validate_string(path, MetadataDiagnosticTarget::album(album_id.clone()), Some("title".to_string()), &string_validator, album.title().as_ref(), report);
    validate_string(path, MetadataDiagnosticTarget::album(album_id.clone()), Some("artist".to_string()), &artist_validator, album.artist(), report);

    if album.artist() == "[Unknown Artist]" || album.artist() == "UnknownArtist" {
        report.add(Diagnostic::error(
            DiagnosticMessage {
                message: "Unknown artist".to_string(),
                target: MetadataDiagnosticTarget::album(album_id.clone()),
            },
            DiagnosticLocation::simple(path.display().to_string()),
        ));
    }

    for (disc_id, disc) in album.discs().iter().enumerate() {
        let disc_id = (disc_id + 1) as u8;

        validate_string(path, MetadataDiagnosticTarget::disc(album_id.clone(), disc_id), Some("title".to_string()), &string_validator, disc.title(), report);
        validate_string(path, MetadataDiagnosticTarget::disc(album_id.clone(), disc_id), Some("artist".to_string()), &artist_validator, disc.artist(), report);

        for (track_id, track) in disc.tracks().iter().enumerate() {
            let track_id = (track_id + 1) as u8;

            validate_string(path, MetadataDiagnosticTarget::track(album_id.clone(), disc_id, track_id), Some("title".to_string()), &string_validator, track.title().as_ref(), report);
            validate_string(path, MetadataDiagnosticTarget::track(album_id.clone(), disc_id, track_id), Some("artist".to_string()), &artist_validator, track.artist(), report);
        }
    }
}

fn validate_string(path: &PathBuf, target: MetadataDiagnosticTarget, field: Option<String>, validator: &ValidatorList, value: &str, report: &mut dyn AnniLinter<MetadataDiagnosticTarget>) {
    validator.validate(value).into_iter().for_each(|(ty, result)| {
        let severity = match result {
            ValidateResult::Warning(_) => DiagnosticSeverity::Warning,
            ValidateResult::Error(_) => DiagnosticSeverity::Error,
            _ => DiagnosticSeverity::Information,
        };
        match result {
            ValidateResult::Warning(message) | ValidateResult::Error(message) => {
                report.add(Diagnostic {
                    severity,
                    message: DiagnosticMessage {
                        message,
                        target: target.clone(),
                    },
                    location: DiagnosticLocation::simple(path.display().to_string()),
                    code: Some(DiagnosticCode::new(format!("{}", ty))),
                    source: None,
                    suggestions: vec![],
                });
            }
            _ => {}
        }
    });
}
