use crate::{ball, fl};
use anni_common::diagnostic::*;
use anni_common::lint::{AnniLinter, AnniLinterReviewDogJsonLineFormat, AnniLinterTextFormat};
use anni_common::validator::{ValidateResult, ValidatorList};
use anni_metadata::model::{Album, DiscRef, UNKNOWN_ARTIST};
use anni_repo::RepositoryManager;
use clap::{Args, ValueEnum};
use clap_handler::handler;
use std::collections::HashSet;
use std::path::Path;

#[derive(Args, Debug, Clone)]
pub struct RepoLintAction {
    #[clap(short, long)]
    #[clap(value_enum, default_value = "text")]
    format: RepoLintFormat,

    albums: Vec<String>,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum RepoLintFormat {
    Text,
    // Markdown,
    #[clap(name = "rdjsonl")]
    ReviewDogJsonLines,
}

#[handler(RepoLintAction)]
fn repo_lint(manager: RepositoryManager, me: &RepoLintAction) -> anyhow::Result<()> {
    info!(target: "anni", "{}", fl!("repo-lint-start"));

    let mut report: Box<dyn AnniLinter<MetadataDiagnosticTarget>> = match me.format {
        RepoLintFormat::Text => Box::new(AnniLinterTextFormat::default()),
        RepoLintFormat::ReviewDogJsonLines => Box::new(AnniLinterReviewDogJsonLineFormat::new()),
    };

    if me.albums.is_empty() {
        // initialize owned manager
        let manager = manager.into_owned_manager()?;
        // validate all albums
        for album in manager.albums_iter() {
            let album_path = manager.album_path(&album.album_id()).unwrap();
            validate_album(album, album_path, report.as_mut());
        }
        // check tag loop
        if let Some(path) = manager.check_tags_loop() {
            report.add(Diagnostic::error(
                DiagnosticMessage {
                    message: format!("Tag loop relation detected: {:?}", path),
                    target: MetadataDiagnosticTarget::Tag(path[0].to_string()),
                },
                DiagnosticLocation::simple(
                    manager.tag_path(&path[0]).unwrap().display().to_string(),
                ),
            ));
        }
    } else {
        // validate selected albums
        for album in me.albums.iter() {
            // FIXME: this may be incorrect
            for (album, path) in manager
                .load_albums(album)?
                .iter()
                .zip(manager.album_paths(album)?)
            {
                validate_album(&album, &path, report.as_mut());
            }
        }
    }

    if !report.flush() {
        ball!("repo-lint-failed");
    }

    info!(target: "anni", "{}", fl!("repo-lint-end"));
    Ok(())
}

fn validate_album<P>(album: &Album, path: P, report: &mut dyn AnniLinter<MetadataDiagnosticTarget>)
where
    P: AsRef<Path>,
{
    let album_id = album.album_id().to_string();

    let string_validator = ValidatorList::new(&["trim", "dot", "tidle"]).unwrap();
    let artist_validator = ValidatorList::new(&["trim", "dot", "tidle", "artist"]).unwrap();

    validate_string(
        path.as_ref(),
        MetadataDiagnosticTarget::album(album_id.clone()),
        Some("title".to_string()),
        &string_validator,
        album.title_raw().as_ref(),
        report,
    );

    if let Some(edition) = album.edition() {
        validate_string(
            path.as_ref(),
            MetadataDiagnosticTarget::album(album_id.clone()),
            Some("edition".to_string()),
            &string_validator,
            edition,
            report,
        );
    }

    validate_string(
        path.as_ref(),
        MetadataDiagnosticTarget::album(album_id.clone()),
        Some("artist".to_string()),
        &artist_validator,
        album.artist(),
        report,
    );

    if album.artist() == UNKNOWN_ARTIST {
        report.add(Diagnostic::error(
            DiagnosticMessage {
                message: "Unknown artist".to_string(),
                target: MetadataDiagnosticTarget::album(album_id.clone()),
            },
            DiagnosticLocation::simple(path.as_ref().display().to_string()),
        ));
    }

    validate_disc_catalog(album.iter().collect(), &album_id, path.as_ref(), report);

    for (disc_id, disc) in album.iter().enumerate() {
        let disc_id = (disc_id + 1) as u8;

        validate_string(
            path.as_ref(),
            MetadataDiagnosticTarget::disc(album_id.clone(), disc_id),
            Some("title".to_string()),
            &string_validator,
            disc.title(),
            report,
        );
        validate_string(
            path.as_ref(),
            MetadataDiagnosticTarget::disc(album_id.clone(), disc_id),
            Some("artist".to_string()),
            &artist_validator,
            disc.artist(),
            report,
        );

        for (track_id, track) in disc.iter().enumerate() {
            let track_id = (track_id + 1) as u8;

            validate_string(
                path.as_ref(),
                MetadataDiagnosticTarget::track(album_id.clone(), disc_id, track_id),
                Some("title".to_string()),
                &string_validator,
                track.title().as_ref(),
                report,
            );
            validate_string(
                path.as_ref(),
                MetadataDiagnosticTarget::track(album_id.clone(), disc_id, track_id),
                Some("artist".to_string()),
                &artist_validator,
                track.artist(),
                report,
            );
        }
    }
}

fn validate_string<P>(
    path: P,
    target: MetadataDiagnosticTarget,
    _field: Option<String>,
    validator: &ValidatorList,
    value: &str,
    report: &mut dyn AnniLinter<MetadataDiagnosticTarget>,
) where
    P: AsRef<Path>,
{
    validator
        .validate(value)
        .into_iter()
        .for_each(|(ty, result)| {
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
                        location: DiagnosticLocation::simple(path.as_ref().display().to_string()),
                        code: Some(DiagnosticCode::new(format!("{}", ty))),
                        source: None,
                        suggestions: vec![],
                    });
                }
                _ => {}
            }
        });
}

fn validate_disc_catalog<P>(
    discs: Vec<DiscRef>,
    album_id: &str,
    path: P,
    report: &mut dyn AnniLinter<MetadataDiagnosticTarget>,
) where
    P: AsRef<Path>,
{
    let mut catalogs = HashSet::new();
    discs.iter().zip(1..).for_each(|(disc, disc_id)| {
        if !catalogs.insert(disc.catalog()) {
            report.add(Diagnostic::warning(
                DiagnosticMessage {
                    target: MetadataDiagnosticTarget::disc(album_id.to_string(), disc_id),
                    message: format!("Duplicate catalog {}", disc.catalog()),
                },
                DiagnosticLocation::simple(path.as_ref().display().to_string()),
            ))
        }
    });
}
