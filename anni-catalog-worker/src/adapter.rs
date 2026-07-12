use std::{
    collections::{hash_map::Entry, HashMap},
    fmt,
    future::Future,
    pin::Pin,
    sync::Arc,
    time::Duration,
};

use anni_catalog::{CatalogSourceKind, SyncCoverage};
use annim::catalog::CatalogSyncLease;
use thiserror::Error;

pub type AdapterFuture<'a> =
    Pin<Box<dyn Future<Output = Result<AdapterPage, AdapterFailure>> + Send + 'a>>;

/// One exact release observation returned by an adapter.
///
/// Neither document is normalized. `raw_document` is the source response (or
/// the exact per-release fragment for a documented API), while
/// `parsed_document` is the adapter's versioned interpretation.
#[derive(Clone, PartialEq, Eq)]
pub struct AdapterObservation {
    pub external_release_id: String,
    pub source_url: String,
    pub raw_document: String,
    pub parsed_document: String,
}

impl fmt::Debug for AdapterObservation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AdapterObservation")
            .field("external_release_id", &"[REDACTED]")
            .field("source_url", &"[REDACTED]")
            .field("raw_document_bytes", &self.raw_document.len())
            .field("parsed_document_bytes", &self.parsed_document.len())
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AdapterPage {
    pub observations: Vec<AdapterObservation>,
    /// Cursor used to fetch the next page of this same run.
    pub next_cursor: Option<String>,
    /// Cursor to seed a future incremental run after this run succeeds.
    pub checkpoint: Option<String>,
    pub coverage: SyncCoverage,
    /// True only when the adapter has traversed the complete declared scope.
    pub complete: bool,
    /// Explicit safety interlock for a legitimate empty full snapshot.
    ///
    /// Adapters must leave this false unless the upstream response positively
    /// proves that the complete source scope contains no releases. A parser
    /// that merely failed to find release nodes must not set it.
    pub empty_full_snapshot_confirmed: bool,
}

impl fmt::Debug for AdapterPage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AdapterPage")
            .field("observation_count", &self.observations.len())
            .field("has_next_cursor", &self.next_cursor.is_some())
            .field("has_checkpoint", &self.checkpoint.is_some())
            .field("coverage", &self.coverage)
            .field("complete", &self.complete)
            .field(
                "empty_full_snapshot_confirmed",
                &self.empty_full_snapshot_confirmed,
            )
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterFailureDisposition {
    Permanent,
    Retryable,
}

/// Stable, persistence-safe adapter failure. Arbitrary remote error bodies,
/// URLs, and credentials never cross this boundary.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct AdapterFailure {
    code: &'static str,
    disposition: AdapterFailureDisposition,
    http_status: Option<u16>,
    retry_after: Option<Duration>,
}

impl AdapterFailure {
    pub fn permanent(code: &'static str, http_status: Option<u16>) -> Self {
        if !valid_failure_code(code) {
            return Self::invalid_code();
        }
        Self {
            code,
            disposition: AdapterFailureDisposition::Permanent,
            http_status,
            retry_after: None,
        }
    }

    pub fn retryable(
        code: &'static str,
        http_status: Option<u16>,
        retry_after: Option<Duration>,
    ) -> Self {
        if !valid_failure_code(code) {
            return Self::invalid_code();
        }
        Self {
            code,
            disposition: AdapterFailureDisposition::Retryable,
            http_status,
            retry_after,
        }
    }

    pub const fn code(self) -> &'static str {
        self.code
    }

    pub const fn disposition(self) -> AdapterFailureDisposition {
        self.disposition
    }

    pub const fn http_status(self) -> Option<u16> {
        self.http_status
    }

    pub const fn retry_after(self) -> Option<Duration> {
        self.retry_after
    }

    pub const fn is_retryable(self) -> bool {
        matches!(self.disposition, AdapterFailureDisposition::Retryable)
    }

    const fn invalid_code() -> Self {
        Self {
            code: "invalid_adapter_failure_code",
            disposition: AdapterFailureDisposition::Permanent,
            http_status: None,
            retry_after: None,
        }
    }
}

fn valid_failure_code(code: &str) -> bool {
    !code.is_empty()
        && code.len() <= 64
        && code
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

impl fmt::Debug for AdapterFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AdapterFailure")
            .field("code", &self.code)
            .field("disposition", &self.disposition)
            .field("http_status", &self.http_status)
            .field("retry_after", &self.retry_after)
            .finish()
    }
}

pub trait CatalogAdapter: Send + Sync + 'static {
    fn source_kind(&self) -> CatalogSourceKind;

    fn fetch_page<'a>(
        &'a self,
        lease: &'a CatalogSyncLease,
        cursor: Option<&'a str>,
    ) -> AdapterFuture<'a>;
}

pub trait CatalogAdapters: Send + Sync {
    fn fetch_page<'a>(
        &'a self,
        lease: &'a CatalogSyncLease,
        cursor: Option<&'a str>,
    ) -> AdapterFuture<'a>;
}

#[derive(Clone, Default)]
pub struct CatalogAdapterRegistry {
    adapters: HashMap<CatalogSourceKind, Arc<dyn CatalogAdapter>>,
}

impl CatalogAdapterRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<A: CatalogAdapter>(&mut self, adapter: A) -> Result<(), AdapterRegistryError> {
        let kind = adapter.source_kind();
        match self.adapters.entry(kind) {
            Entry::Vacant(entry) => {
                entry.insert(Arc::new(adapter));
                Ok(())
            }
            Entry::Occupied(_) => Err(AdapterRegistryError::DuplicateAdapter { kind }),
        }
    }

    pub fn contains(&self, kind: CatalogSourceKind) -> bool {
        self.adapters.contains_key(&kind)
    }
}

impl CatalogAdapters for CatalogAdapterRegistry {
    fn fetch_page<'a>(
        &'a self,
        lease: &'a CatalogSyncLease,
        cursor: Option<&'a str>,
    ) -> AdapterFuture<'a> {
        let Some(adapter) = self.adapters.get(&lease.kind) else {
            return Box::pin(async { Err(AdapterFailure::permanent("adapter_unavailable", None)) });
        };
        adapter.fetch_page(lease, cursor)
    }
}

impl fmt::Debug for CatalogAdapterRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut kinds: Vec<_> = self.adapters.keys().copied().collect();
        kinds.sort_by_key(|kind| kind.as_str());
        formatter
            .debug_struct("CatalogAdapterRegistry")
            .field("source_kinds", &kinds)
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum AdapterRegistryError {
    #[error("an adapter is already registered for {kind}")]
    DuplicateAdapter { kind: CatalogSourceKind },
}
