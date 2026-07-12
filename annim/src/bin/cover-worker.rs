use std::{path::PathBuf, time::Duration};

use anni_cover_worker::{AssetStore, CoverDownloader};
use annim::{
    cover::{CoverRepository, CoverService},
    cover_worker::{CoverWorker, CoverWorkerOutcome, CoverWorkerPolicy, SafeCoverFetcher},
};
use sea_orm::Database;
use thiserror::Error;

const DEFAULT_POLL_INTERVAL_SECONDS: u64 = 5;
const MAX_POLL_INTERVAL_SECONDS: u64 = 3_600;

struct WorkerConfig {
    asset_root: PathBuf,
    poll_interval: Duration,
}

impl WorkerConfig {
    fn from_env() -> Result<Self, WorkerConfigError> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    fn from_lookup(
        mut lookup: impl FnMut(&str) -> Option<String>,
    ) -> Result<Self, WorkerConfigError> {
        let asset_root = lookup("ANNIM_COVER_ASSET_ROOT")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .ok_or(WorkerConfigError::MissingAssetRoot)?;
        let poll_seconds = match lookup("ANNIM_COVER_POLL_INTERVAL_SECONDS") {
            None => DEFAULT_POLL_INTERVAL_SECONDS,
            Some(value) => value
                .parse::<u64>()
                .ok()
                .filter(|seconds| (1..=MAX_POLL_INTERVAL_SECONDS).contains(seconds))
                .ok_or(WorkerConfigError::InvalidPollInterval)?,
        };
        Ok(Self {
            asset_root,
            poll_interval: Duration::from_secs(poll_seconds),
        })
    }
}

#[derive(Debug, Error)]
enum WorkerConfigError {
    #[error("ANNIM_COVER_ASSET_ROOT is required")]
    MissingAssetRoot,
    #[error("ANNIM_COVER_POLL_INTERVAL_SECONDS must be an integer between 1 and 3600")]
    InvalidPollInterval,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_test_writer()
        .init();

    let database_url = std::env::var("ANNIM_DATABASE_URL")?;
    let config = WorkerConfig::from_env()?;
    let database = Database::connect(database_url).await?;
    let covers = CoverService::new(CoverRepository::new(database));
    // `open` requires a pre-existing trusted directory and verifies its
    // canonical location before any candidate is claimed.
    let asset_store = AssetStore::open(&config.asset_root)?;
    let fetcher = SafeCoverFetcher::new(CoverDownloader::default(), asset_store);
    let worker = CoverWorker::new(covers, fetcher, CoverWorkerPolicy::default())?;

    tracing::info!("Annim cover worker started");
    loop {
        let cycle = tokio::select! {
            signal = tokio::signal::ctrl_c() => {
                signal?;
                break;
            }
            result = worker.run_once() => result,
        };

        let should_wait = match cycle {
            Ok(CoverWorkerOutcome::Idle) => true,
            Ok(CoverWorkerOutcome::Verified {
                candidate_id,
                attempt_count,
            }) => {
                tracing::info!(%candidate_id, attempt_count, "cover candidate verified");
                false
            }
            Ok(CoverWorkerOutcome::RetryScheduled {
                candidate_id,
                attempt_count,
                not_before,
                problem,
            }) => {
                tracing::warn!(
                    %candidate_id,
                    attempt_count,
                    code = problem.code(),
                    http_status = problem.http_status(),
                    %not_before,
                    "cover candidate retry scheduled"
                );
                false
            }
            Ok(CoverWorkerOutcome::Rejected {
                candidate_id,
                attempt_count,
                reason,
                problem,
            }) => {
                tracing::warn!(
                    %candidate_id,
                    attempt_count,
                    ?reason,
                    code = problem.code(),
                    http_status = problem.http_status(),
                    "cover candidate rejected"
                );
                false
            }
            Err(error) => {
                // CoverWorkerRunError has a deliberately generic Display for
                // queue failures. Do not switch this to Debug: underlying DB
                // diagnostics are not part of the worker log contract.
                tracing::error!(error = %error, "cover worker cycle failed");
                true
            }
        };

        if should_wait {
            tokio::select! {
                signal = tokio::signal::ctrl_c() => {
                    signal?;
                    break;
                }
                _ = tokio::time::sleep(config.poll_interval) => {}
            }
        }
    }
    tracing::info!("Annim cover worker stopped");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_configuration_is_explicit_and_bounded() {
        assert!(matches!(
            WorkerConfig::from_lookup(|_| None),
            Err(WorkerConfigError::MissingAssetRoot)
        ));

        let configured = WorkerConfig::from_lookup(|name| match name {
            "ANNIM_COVER_ASSET_ROOT" => Some("/trusted/covers".to_owned()),
            "ANNIM_COVER_POLL_INTERVAL_SECONDS" => Some("30".to_owned()),
            _ => None,
        })
        .unwrap();
        assert_eq!(configured.asset_root, PathBuf::from("/trusted/covers"));
        assert_eq!(configured.poll_interval, Duration::from_secs(30));

        for invalid in ["0", "3601", "not-a-number"] {
            assert!(matches!(
                WorkerConfig::from_lookup(|name| match name {
                    "ANNIM_COVER_ASSET_ROOT" => Some("/trusted/covers".to_owned()),
                    "ANNIM_COVER_POLL_INTERVAL_SECONDS" => Some(invalid.to_owned()),
                    _ => None,
                }),
                Err(WorkerConfigError::InvalidPollInterval)
            ));
        }
    }
}
