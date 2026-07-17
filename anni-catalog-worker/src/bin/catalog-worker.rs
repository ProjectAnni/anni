//! Executable catalog synchronization worker.
//!
//! This binary lives in `anni-catalog-worker` rather than `annim`: the worker
//! library already depends on Annim's durable queue, so reversing that edge
//! would create a Cargo dependency cycle.

use std::{
    fmt,
    fs::{self, File},
    io::Read,
    path::{Component, Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anni_catalog_worker::{
    AdapterFailure, AppleMusicAdapter, AppleMusicHttpPolicy, CatalogAdapterRegistry, CatalogWorker,
    CatalogWorkerOutcome, CatalogWorkerPolicy, SecretFuture, SecretResolver, SecretValue,
};
use annim::{catalog::CatalogSyncService, migrator::Migrator};
use sea_orm::{ConnectOptions, Database};
use sea_orm_migration::MigratorTrait;
use thiserror::Error;

const DEFAULT_POLL_SECONDS: u64 = 5;
const DEFAULT_LEASE_SECONDS: u64 = 10 * 60;
const DEFAULT_PAGE_TIMEOUT_SECONDS: u64 = 60;
const DEFAULT_CONNECT_TIMEOUT_SECONDS: u64 = 10;
const DEFAULT_REQUEST_TIMEOUT_SECONDS: u64 = 30;
const DEFAULT_RESPONSE_BYTES: u64 = 4 * 1024 * 1024;
const DEFAULT_SECRET_BYTES: u64 = 16 * 1024;

const MAX_DATABASE_URL_BYTES: usize = 4 * 1024;
const MAX_SECRET_ROOT_BYTES: usize = 4 * 1024;
const MAX_POLL_SECONDS: u64 = 60 * 60;
const MIN_LEASE_SECONDS: u64 = 30;
const MAX_LEASE_SECONDS: u64 = 60 * 60;
const MAX_PAGE_TIMEOUT_SECONDS: u64 = 10 * 60;
const MAX_CONNECT_TIMEOUT_SECONDS: u64 = 60;
const MAX_REQUEST_TIMEOUT_SECONDS: u64 = 5 * 60;
const MIN_RESPONSE_BYTES: u64 = 64 * 1024;
const MAX_RESPONSE_BYTES: u64 = 32 * 1024 * 1024;
const MIN_SECRET_BYTES: u64 = 256;
const MAX_SECRET_BYTES: u64 = 64 * 1024;

struct WorkerConfig {
    database_url: String,
    secret_root: PathBuf,
    poll_interval: Duration,
    worker_policy: CatalogWorkerPolicy,
    http_policy: AppleMusicHttpPolicy,
    max_secret_bytes: usize,
}

impl WorkerConfig {
    fn from_env() -> Result<Self, WorkerConfigError> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    fn from_lookup(
        mut lookup: impl FnMut(&str) -> Option<String>,
    ) -> Result<Self, WorkerConfigError> {
        let database_url =
            required_text(&mut lookup, "ANNIM_DATABASE_URL", MAX_DATABASE_URL_BYTES)?;
        let secret_root_value = required_text(
            &mut lookup,
            "ANNIM_CATALOG_SECRET_ROOT",
            MAX_SECRET_ROOT_BYTES,
        )?;
        let secret_root = PathBuf::from(secret_root_value);
        if !secret_root.is_absolute() {
            return Err(WorkerConfigError::InvalidValue {
                name: "ANNIM_CATALOG_SECRET_ROOT",
                requirement: "must be an absolute path",
            });
        }

        let poll_seconds = bounded_integer(
            &mut lookup,
            "ANNIM_CATALOG_POLL_INTERVAL_SECONDS",
            DEFAULT_POLL_SECONDS,
            1,
            MAX_POLL_SECONDS,
        )?;
        let lease_seconds = bounded_integer(
            &mut lookup,
            "ANNIM_CATALOG_LEASE_SECONDS",
            DEFAULT_LEASE_SECONDS,
            MIN_LEASE_SECONDS,
            MAX_LEASE_SECONDS,
        )?;
        let page_timeout_seconds = bounded_integer(
            &mut lookup,
            "ANNIM_CATALOG_PAGE_TIMEOUT_SECONDS",
            DEFAULT_PAGE_TIMEOUT_SECONDS,
            2,
            MAX_PAGE_TIMEOUT_SECONDS,
        )?;
        let connect_timeout_seconds = bounded_integer(
            &mut lookup,
            "ANNIM_CATALOG_CONNECT_TIMEOUT_SECONDS",
            DEFAULT_CONNECT_TIMEOUT_SECONDS,
            1,
            MAX_CONNECT_TIMEOUT_SECONDS,
        )?;
        let request_timeout_seconds = bounded_integer(
            &mut lookup,
            "ANNIM_CATALOG_REQUEST_TIMEOUT_SECONDS",
            DEFAULT_REQUEST_TIMEOUT_SECONDS,
            1,
            MAX_REQUEST_TIMEOUT_SECONDS,
        )?;
        let max_response_bytes = bounded_integer(
            &mut lookup,
            "ANNIM_CATALOG_MAX_RESPONSE_BYTES",
            DEFAULT_RESPONSE_BYTES,
            MIN_RESPONSE_BYTES,
            MAX_RESPONSE_BYTES,
        )?;
        let max_secret_bytes = bounded_integer(
            &mut lookup,
            "ANNIM_CATALOG_MAX_SECRET_BYTES",
            DEFAULT_SECRET_BYTES,
            MIN_SECRET_BYTES,
            MAX_SECRET_BYTES,
        )?;

        if connect_timeout_seconds > request_timeout_seconds {
            return Err(WorkerConfigError::InvalidTimeoutOrder {
                shorter: "ANNIM_CATALOG_CONNECT_TIMEOUT_SECONDS",
                longer: "ANNIM_CATALOG_REQUEST_TIMEOUT_SECONDS",
            });
        }
        if request_timeout_seconds >= page_timeout_seconds {
            return Err(WorkerConfigError::InvalidTimeoutOrder {
                shorter: "ANNIM_CATALOG_REQUEST_TIMEOUT_SECONDS",
                longer: "ANNIM_CATALOG_PAGE_TIMEOUT_SECONDS",
            });
        }
        if page_timeout_seconds >= lease_seconds {
            return Err(WorkerConfigError::InvalidTimeoutOrder {
                shorter: "ANNIM_CATALOG_PAGE_TIMEOUT_SECONDS",
                longer: "ANNIM_CATALOG_LEASE_SECONDS",
            });
        }

        let worker_policy = CatalogWorkerPolicy {
            lease_for: Duration::from_secs(lease_seconds),
            page_timeout: Duration::from_secs(page_timeout_seconds),
            ..CatalogWorkerPolicy::default()
        };
        let http_policy = AppleMusicHttpPolicy {
            connect_timeout: Duration::from_secs(connect_timeout_seconds),
            request_timeout: Duration::from_secs(request_timeout_seconds),
            max_response_bytes,
        };
        Ok(Self {
            database_url,
            secret_root,
            poll_interval: Duration::from_secs(poll_seconds),
            worker_policy,
            http_policy,
            max_secret_bytes: max_secret_bytes as usize,
        })
    }
}

impl fmt::Debug for WorkerConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkerConfig")
            .field("database_url", &"[REDACTED]")
            .field("secret_root", &"[REDACTED]")
            .field("poll_interval", &self.poll_interval)
            .field("worker_policy", &self.worker_policy)
            .field("http_policy", &self.http_policy)
            .field("max_secret_bytes", &self.max_secret_bytes)
            .finish()
    }
}

fn required_text(
    lookup: &mut impl FnMut(&str) -> Option<String>,
    name: &'static str,
    max_bytes: usize,
) -> Result<String, WorkerConfigError> {
    let value = lookup(name).ok_or(WorkerConfigError::MissingValue { name })?;
    if value.is_empty() || value.len() > max_bytes || value.chars().any(char::is_control) {
        return Err(WorkerConfigError::InvalidValue {
            name,
            requirement: "must be non-empty, bounded UTF-8 without control characters",
        });
    }
    Ok(value)
}

fn bounded_integer(
    lookup: &mut impl FnMut(&str) -> Option<String>,
    name: &'static str,
    default: u64,
    minimum: u64,
    maximum: u64,
) -> Result<u64, WorkerConfigError> {
    match lookup(name) {
        None => Ok(default),
        Some(value) => value
            .parse::<u64>()
            .ok()
            .filter(|value| (minimum..=maximum).contains(value))
            .ok_or(WorkerConfigError::InvalidValue {
                name,
                requirement: "must be an integer within the documented bounds",
            }),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
enum WorkerConfigError {
    #[error("required worker setting {name} is missing")]
    MissingValue { name: &'static str },
    #[error("invalid worker setting {name}: {requirement}")]
    InvalidValue {
        name: &'static str,
        requirement: &'static str,
    },
    #[error("{shorter} must be shorter than {longer}")]
    InvalidTimeoutOrder {
        shorter: &'static str,
        longer: &'static str,
    },
}

#[derive(Clone)]
struct FileSecretResolver {
    canonical_root: Arc<PathBuf>,
    max_bytes: usize,
}

impl FileSecretResolver {
    fn open(root: &Path, max_bytes: usize) -> Result<Self, SecretFileError> {
        if max_bytes == 0 {
            return Err(SecretFileError::InvalidSizeLimit);
        }
        let canonical_root =
            fs::canonicalize(root).map_err(|_| SecretFileError::RootUnavailable)?;
        let metadata =
            fs::metadata(&canonical_root).map_err(|_| SecretFileError::RootUnavailable)?;
        if !metadata.is_dir() {
            return Err(SecretFileError::RootNotDirectory);
        }
        Ok(Self {
            canonical_root: Arc::new(canonical_root),
            max_bytes,
        })
    }

    fn read_secret(&self, secret_ref: &str) -> Result<SecretValue, SecretFileError> {
        let components = validate_secret_reference(secret_ref)?;
        let mut candidate = self.canonical_root.as_ref().clone();
        for (index, component) in components.iter().enumerate() {
            candidate.push(component);
            let metadata =
                fs::symlink_metadata(&candidate).map_err(|_| SecretFileError::SecretUnavailable)?;
            if metadata.file_type().is_symlink() {
                return Err(SecretFileError::SymlinkNotAllowed);
            }
            if index + 1 == components.len() {
                if !metadata.is_file() {
                    return Err(SecretFileError::SecretNotRegularFile);
                }
            } else if !metadata.is_dir() {
                return Err(SecretFileError::SecretUnavailable);
            }
        }

        let canonical =
            fs::canonicalize(&candidate).map_err(|_| SecretFileError::SecretUnavailable)?;
        if !canonical.starts_with(self.canonical_root.as_ref()) {
            return Err(SecretFileError::ReferenceEscapesRoot);
        }
        let metadata = fs::metadata(&canonical).map_err(|_| SecretFileError::SecretUnavailable)?;
        if !metadata.is_file() {
            return Err(SecretFileError::SecretNotRegularFile);
        }
        if metadata.len() > self.max_bytes as u64 {
            return Err(SecretFileError::SecretTooLarge);
        }

        let file = File::open(&canonical).map_err(|_| SecretFileError::SecretUnavailable)?;
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        file.take(self.max_bytes as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| SecretFileError::SecretUnavailable)?;
        if bytes.len() > self.max_bytes {
            return Err(SecretFileError::SecretTooLarge);
        }
        let value = String::from_utf8(bytes).map_err(|_| SecretFileError::SecretNotUtf8)?;
        SecretValue::new(value).map_err(|_| SecretFileError::SecretInvalid)
    }
}

impl fmt::Debug for FileSecretResolver {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FileSecretResolver")
            .field("canonical_root", &"[REDACTED]")
            .field("max_bytes", &self.max_bytes)
            .finish()
    }
}

impl SecretResolver for FileSecretResolver {
    fn resolve<'a>(&'a self, secret_ref: &'a str) -> SecretFuture<'a> {
        let resolver = self.clone();
        let secret_ref = secret_ref.to_owned();
        Box::pin(async move {
            let result = tokio::task::spawn_blocking(move || resolver.read_secret(&secret_ref))
                .await
                .map_err(|_| {
                    AdapterFailure::retryable("apple_secret_resolver_failed", None, None)
                })?;
            result.map_err(SecretFileError::adapter_failure)
        })
    }
}

fn validate_secret_reference(secret_ref: &str) -> Result<Vec<&std::ffi::OsStr>, SecretFileError> {
    if secret_ref.is_empty()
        || secret_ref.len() > 512
        || secret_ref.contains('\\')
        || secret_ref.chars().any(char::is_control)
    {
        return Err(SecretFileError::InvalidReference);
    }
    let path = Path::new(secret_ref);
    if path.is_absolute() {
        return Err(SecretFileError::InvalidReference);
    }
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(component) => components.push(component),
            Component::CurDir
            | Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_) => return Err(SecretFileError::InvalidReference),
        }
    }
    if components.is_empty() {
        return Err(SecretFileError::InvalidReference);
    }
    Ok(components)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
enum SecretFileError {
    #[error("the catalog secret size limit is invalid")]
    InvalidSizeLimit,
    #[error("the catalog secret root is unavailable")]
    RootUnavailable,
    #[error("the catalog secret root is not a directory")]
    RootNotDirectory,
    #[error("the catalog secret reference is invalid")]
    InvalidReference,
    #[error("catalog secret references may not contain symbolic links")]
    SymlinkNotAllowed,
    #[error("the catalog secret reference escapes its root")]
    ReferenceEscapesRoot,
    #[error("the catalog secret is unavailable")]
    SecretUnavailable,
    #[error("the catalog secret is not a regular file")]
    SecretNotRegularFile,
    #[error("the catalog secret exceeds the configured size limit")]
    SecretTooLarge,
    #[error("the catalog secret is not UTF-8")]
    SecretNotUtf8,
    #[error("the catalog secret is not a valid developer token")]
    SecretInvalid,
}

impl SecretFileError {
    fn adapter_failure(self) -> AdapterFailure {
        match self {
            Self::RootUnavailable | Self::SecretUnavailable => {
                AdapterFailure::retryable("apple_secret_unavailable", None, None)
            }
            Self::InvalidSizeLimit
            | Self::RootNotDirectory
            | Self::InvalidReference
            | Self::SymlinkNotAllowed
            | Self::ReferenceEscapesRoot
            | Self::SecretNotRegularFile
            | Self::SecretTooLarge
            | Self::SecretNotUtf8
            | Self::SecretInvalid => AdapterFailure::permanent("apple_secret_invalid", None),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
enum ProcessError {
    #[error("catalog worker configuration is invalid")]
    Configuration,
    #[error("catalog secret resolver initialization failed")]
    SecretResolver,
    #[error("catalog database connection failed")]
    DatabaseConnection,
    #[error("catalog database migration failed")]
    DatabaseMigration,
    #[error("Apple Music transport initialization failed")]
    Transport,
    #[error("catalog adapter registration failed")]
    AdapterRegistration,
    #[error("catalog worker initialization failed")]
    WorkerInitialization,
    #[error("catalog worker shutdown signal failed")]
    ShutdownSignal,
}

#[tokio::main]
async fn main() -> Result<(), ProcessError> {
    tracing_subscriber::fmt()
        // Do not inherit RUST_LOG: dependency diagnostics can contain SQL or
        // request context outside this binary's deliberately small log
        // contract.
        .with_env_filter("off,annim_catalog_worker=info")
        .init();

    let config = WorkerConfig::from_env().map_err(|_| ProcessError::Configuration)?;
    let secrets = Arc::new(
        FileSecretResolver::open(&config.secret_root, config.max_secret_bytes)
            .map_err(|_| ProcessError::SecretResolver)?,
    );
    let mut database_options = ConnectOptions::new(config.database_url);
    database_options.sqlx_logging(false);
    let database = Database::connect(database_options)
        .await
        .map_err(|_| ProcessError::DatabaseConnection)?;
    Migrator::up(&database, None)
        .await
        .map_err(|_| ProcessError::DatabaseMigration)?;

    let apple_music =
        AppleMusicAdapter::new(secrets, config.http_policy).map_err(|_| ProcessError::Transport)?;
    let mut adapters = CatalogAdapterRegistry::new();
    adapters
        .register(apple_music)
        .map_err(|_| ProcessError::AdapterRegistration)?;
    let worker = CatalogWorker::new(
        CatalogSyncService::new(database),
        adapters,
        config.worker_policy,
    )
    .map_err(|_| ProcessError::WorkerInitialization)?;

    tracing::info!(target: "annim_catalog_worker", "Annim catalog worker started");
    loop {
        let cycle = tokio::select! {
            signal = tokio::signal::ctrl_c() => {
                signal.map_err(|_| ProcessError::ShutdownSignal)?;
                break;
            }
            result = worker.run_once() => result,
        };

        let should_wait = match cycle {
            Ok(CatalogWorkerOutcome::Idle) => true,
            Ok(CatalogWorkerOutcome::Succeeded { run_id, .. }) => {
                tracing::info!(target: "annim_catalog_worker", %run_id, "catalog sync run succeeded");
                false
            }
            Ok(CatalogWorkerOutcome::RetryScheduled {
                run_id, failure, ..
            }) => {
                tracing::warn!(target: "annim_catalog_worker", %run_id, code = failure.code(), "catalog sync retry scheduled");
                false
            }
            Ok(CatalogWorkerOutcome::Failed {
                run_id, failure, ..
            }) => {
                tracing::warn!(target: "annim_catalog_worker", %run_id, code = failure.code(), "catalog sync run failed");
                false
            }
            Err(_) => {
                // Queue errors may wrap database diagnostics. Never log their
                // Debug representation; this stable code is the entire log
                // contract for an unowned cycle failure.
                tracing::error!(
                    target: "annim_catalog_worker",
                    code = "catalog_worker_cycle_failed",
                    "catalog worker cycle failed"
                );
                true
            }
        };

        if should_wait {
            tokio::select! {
                signal = tokio::signal::ctrl_c() => {
                    signal.map_err(|_| ProcessError::ShutdownSignal)?;
                    break;
                }
                _ = tokio::time::sleep(config.poll_interval) => {}
            }
        }
    }
    tracing::info!(target: "annim_catalog_worker", "Annim catalog worker stopped");
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use tempfile::tempdir;

    use super::*;

    const TOKEN: &str = "eyJhbGciOiJFUzI1NiJ9.eyJpc3MiOiJURUFNSUQifQ.c2lnbmF0dXJl";

    fn valid_settings(secret_root: &Path) -> HashMap<&'static str, String> {
        HashMap::from([
            (
                "ANNIM_DATABASE_URL",
                "postgres://user:database-password@example.invalid/annim".to_owned(),
            ),
            (
                "ANNIM_CATALOG_SECRET_ROOT",
                secret_root.to_string_lossy().into_owned(),
            ),
            ("ANNIM_CATALOG_POLL_INTERVAL_SECONDS", "7".to_owned()),
            ("ANNIM_CATALOG_LEASE_SECONDS", "300".to_owned()),
            ("ANNIM_CATALOG_PAGE_TIMEOUT_SECONDS", "45".to_owned()),
            ("ANNIM_CATALOG_CONNECT_TIMEOUT_SECONDS", "5".to_owned()),
            ("ANNIM_CATALOG_REQUEST_TIMEOUT_SECONDS", "20".to_owned()),
            (
                "ANNIM_CATALOG_MAX_RESPONSE_BYTES",
                (2 * 1024 * 1024).to_string(),
            ),
            ("ANNIM_CATALOG_MAX_SECRET_BYTES", (8 * 1024).to_string()),
        ])
    }

    #[test]
    fn configuration_is_bounded_ordered_and_redacted() {
        let root = tempdir().unwrap();
        let values = valid_settings(root.path());
        let config = WorkerConfig::from_lookup(|name| values.get(name).cloned()).unwrap();
        assert_eq!(config.poll_interval, Duration::from_secs(7));
        assert_eq!(config.worker_policy.lease_for, Duration::from_secs(300));
        assert_eq!(config.worker_policy.page_timeout, Duration::from_secs(45));
        assert_eq!(config.http_policy.request_timeout, Duration::from_secs(20));
        assert!(config.http_policy.request_timeout < config.worker_policy.lease_for);
        let debug = format!("{config:?}");
        assert!(!debug.contains("database-password"));
        assert!(!debug.contains(root.path().to_string_lossy().as_ref()));

        assert!(matches!(
            WorkerConfig::from_lookup(|_| None),
            Err(WorkerConfigError::MissingValue {
                name: "ANNIM_DATABASE_URL"
            })
        ));

        let mut values = valid_settings(root.path());
        values.insert("ANNIM_CATALOG_SECRET_ROOT", "relative/secrets".to_owned());
        assert!(matches!(
            WorkerConfig::from_lookup(|name| values.get(name).cloned()),
            Err(WorkerConfigError::InvalidValue {
                name: "ANNIM_CATALOG_SECRET_ROOT",
                ..
            })
        ));

        let mut values = valid_settings(root.path());
        values.insert("ANNIM_CATALOG_REQUEST_TIMEOUT_SECONDS", "45".to_owned());
        assert!(matches!(
            WorkerConfig::from_lookup(|name| values.get(name).cloned()),
            Err(WorkerConfigError::InvalidTimeoutOrder { .. })
        ));

        let mut values = valid_settings(root.path());
        values.insert(
            "ANNIM_CATALOG_MAX_RESPONSE_BYTES",
            (MAX_RESPONSE_BYTES + 1).to_string(),
        );
        assert!(matches!(
            WorkerConfig::from_lookup(|name| values.get(name).cloned()),
            Err(WorkerConfigError::InvalidValue {
                name: "ANNIM_CATALOG_MAX_RESPONSE_BYTES",
                ..
            })
        ));
    }

    #[tokio::test]
    async fn file_secret_resolver_is_bounded_and_redacted() {
        let temporary = tempdir().unwrap();
        let root = temporary.path().join("secrets");
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("apple")).unwrap();
        fs::write(root.join("apple/developer.jwt"), TOKEN).unwrap();
        fs::write(root.join("too-large.jwt"), "x".repeat(129)).unwrap();
        let resolver = FileSecretResolver::open(&root, 128).unwrap();

        let secret = resolver.read_secret("apple/developer.jwt").unwrap();
        assert_eq!(secret.expose_secret(), TOKEN);
        assert!(!format!("{resolver:?}").contains(root.to_string_lossy().as_ref()));
        assert!(!format!("{secret:?}").contains(TOKEN));
        assert_eq!(
            resolver.read_secret("too-large.jwt").unwrap_err(),
            SecretFileError::SecretTooLarge
        );

        for invalid in ["../outside.jwt", "/outside.jwt", "apple\\developer.jwt"] {
            let error = resolver.read_secret(invalid).unwrap_err();
            assert_eq!(error, SecretFileError::InvalidReference);
            assert!(!format!("{error:?}").contains(invalid));
        }
        let failure = resolver.resolve("missing.jwt").await.unwrap_err();
        assert_eq!(failure.code(), "apple_secret_unavailable");
        assert!(!format!("{failure:?}").contains("missing.jwt"));
    }

    #[cfg(unix)]
    #[test]
    fn file_secret_resolver_rejects_symlink_escape() {
        use std::os::unix::fs::symlink;

        let temporary = tempdir().unwrap();
        let root = temporary.path().join("secrets");
        fs::create_dir(&root).unwrap();
        let outside = temporary.path().join("outside.jwt");
        fs::write(&outside, TOKEN).unwrap();
        symlink(&outside, root.join("escaped.jwt")).unwrap();
        let outside_directory = temporary.path().join("outside-secrets");
        fs::create_dir(&outside_directory).unwrap();
        fs::write(outside_directory.join("developer.jwt"), TOKEN).unwrap();
        symlink(&outside_directory, root.join("escaped-directory")).unwrap();
        let resolver = FileSecretResolver::open(&root, 1024).unwrap();

        for reference in ["escaped.jwt", "escaped-directory/developer.jwt"] {
            assert_eq!(
                resolver.read_secret(reference).unwrap_err(),
                SecretFileError::SymlinkNotAllowed
            );
        }
    }
}
