//! Browser-safe administration of external catalog synchronization.
//!
//! This module intentionally exposes only control-plane operations. Source
//! locators, adapter configuration, secret references, cursors, raw responses,
//! parsed responses, and worker lifecycle commands stay outside the Web API.

use std::fmt;

use anni_catalog::{
    CatalogSourceKind as DomainCatalogSourceKind, SyncCoverage as DomainSyncCoverage,
    SyncRunStatus as DomainSyncRunStatus,
};
use async_graphql::{Context, Enum, Error, ErrorExtensions, InputObject, Result, SimpleObject};
use chrono::{DateTime, Utc};
use sea_orm::prelude::Uuid;

use crate::catalog::{
    CatalogSourceProvisioningState as DomainCatalogSourceProvisioningState, CatalogSourceSnapshot,
    CatalogSyncError, CatalogSyncRunSnapshot, CatalogSyncService, NewCatalogSyncRun,
    NewManagedCatalogSource,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum CatalogSyncSourceKind {
    AppleMusic,
    RecordLabel,
    ArtistWebsite,
    Vgmdb,
    Manual,
}

impl From<DomainCatalogSourceKind> for CatalogSyncSourceKind {
    fn from(value: DomainCatalogSourceKind) -> Self {
        match value {
            DomainCatalogSourceKind::AppleMusic => Self::AppleMusic,
            DomainCatalogSourceKind::RecordLabel => Self::RecordLabel,
            DomainCatalogSourceKind::ArtistWebsite => Self::ArtistWebsite,
            DomainCatalogSourceKind::Vgmdb => Self::Vgmdb,
            DomainCatalogSourceKind::Manual => Self::Manual,
        }
    }
}

impl From<CatalogSyncSourceKind> for DomainCatalogSourceKind {
    fn from(value: CatalogSyncSourceKind) -> Self {
        match value {
            CatalogSyncSourceKind::AppleMusic => Self::AppleMusic,
            CatalogSyncSourceKind::RecordLabel => Self::RecordLabel,
            CatalogSyncSourceKind::ArtistWebsite => Self::ArtistWebsite,
            CatalogSyncSourceKind::Vgmdb => Self::Vgmdb,
            CatalogSyncSourceKind::Manual => Self::Manual,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum CatalogSyncRunStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

impl From<DomainSyncRunStatus> for CatalogSyncRunStatus {
    fn from(value: DomainSyncRunStatus) -> Self {
        match value {
            DomainSyncRunStatus::Queued => Self::Queued,
            DomainSyncRunStatus::Running => Self::Running,
            DomainSyncRunStatus::Succeeded => Self::Succeeded,
            DomainSyncRunStatus::Failed => Self::Failed,
            DomainSyncRunStatus::Cancelled => Self::Cancelled,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum CatalogSyncCoverage {
    FullSnapshot,
    Incremental,
    DiscoveryOnly,
}

/// Whether this server can safely queue work for a source.
///
/// This reports deployment capability without exposing the credential
/// reference or assuming that the referenced token itself is valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum CatalogSyncProvisioningState {
    ReadyToQueue,
    Disabled,
    CredentialNotConfigured,
    CredentialBindingInvalid,
    AdapterUnavailable,
}

impl From<DomainCatalogSourceProvisioningState> for CatalogSyncProvisioningState {
    fn from(value: DomainCatalogSourceProvisioningState) -> Self {
        match value {
            DomainCatalogSourceProvisioningState::ReadyToQueue => Self::ReadyToQueue,
            DomainCatalogSourceProvisioningState::Disabled => Self::Disabled,
            DomainCatalogSourceProvisioningState::CredentialNotConfigured => {
                Self::CredentialNotConfigured
            }
            DomainCatalogSourceProvisioningState::CredentialBindingInvalid => {
                Self::CredentialBindingInvalid
            }
            DomainCatalogSourceProvisioningState::AdapterUnavailable => Self::AdapterUnavailable,
        }
    }
}

impl From<DomainSyncCoverage> for CatalogSyncCoverage {
    fn from(value: DomainSyncCoverage) -> Self {
        match value {
            DomainSyncCoverage::FullSnapshot => Self::FullSnapshot,
            DomainSyncCoverage::Incremental => Self::Incremental,
            DomainSyncCoverage::DiscoveryOnly => Self::DiscoveryOnly,
        }
    }
}

/// Source metadata safe for an authenticated browser administrator.
///
/// The source locator is deliberately not represented. It may be a signed URL
/// or otherwise contain credentials even when the source kind is public.
#[derive(Debug, Clone, PartialEq, Eq, SimpleObject)]
pub struct CatalogSyncSourceInfo {
    source_id: Uuid,
    artist_id: Uuid,
    kind: CatalogSyncSourceKind,
    storefront: Option<String>,
    locale: Option<String>,
    enabled: bool,
    provisioning_state: CatalogSyncProvisioningState,
    row_version: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<CatalogSourceSnapshot> for CatalogSyncSourceInfo {
    fn from(value: CatalogSourceSnapshot) -> Self {
        Self {
            source_id: value.source_id,
            artist_id: value.artist_id,
            kind: value.kind.into(),
            storefront: value.storefront,
            locale: value.locale,
            enabled: value.enabled,
            provisioning_state: value.provisioning_state.into(),
            row_version: value.row_version.to_string(),
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

/// Run progress safe for polling from the browser.
///
/// Cursors and adapter error text are deliberately omitted. Counters and row
/// versions use base-10 strings to avoid GraphQL and JavaScript integer loss.
#[derive(Debug, Clone, PartialEq, Eq, SimpleObject)]
pub struct CatalogSyncRunInfo {
    run_id: Uuid,
    source_id: Uuid,
    status: CatalogSyncRunStatus,
    coverage: CatalogSyncCoverage,
    started_from_root: bool,
    snapshot_complete: bool,
    observed_count: String,
    attempt_count: String,
    next_attempt_at: Option<DateTime<Utc>>,
    row_version: String,
    created_at: DateTime<Utc>,
    started_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
}

impl From<CatalogSyncRunSnapshot> for CatalogSyncRunInfo {
    fn from(value: CatalogSyncRunSnapshot) -> Self {
        Self {
            run_id: value.run_id,
            source_id: value.source_id,
            status: value.status.into(),
            coverage: value.coverage.into(),
            started_from_root: value.started_from_root,
            snapshot_complete: value.snapshot_complete,
            observed_count: value.observed_count.to_string(),
            attempt_count: value.attempt_count.to_string(),
            next_attempt_at: value.next_attempt_at,
            row_version: value.row_version.to_string(),
            created_at: value.created_at,
            started_at: value.started_at,
            finished_at: value.finished_at,
        }
    }
}

#[derive(Clone, PartialEq, Eq, InputObject)]
pub struct CreateCatalogSyncSourceInput {
    source_id: Option<Uuid>,
    artist_id: Uuid,
    kind: CatalogSyncSourceKind,
    locator: String,
    storefront: Option<String>,
    locale: Option<String>,
}

impl fmt::Debug for CreateCatalogSyncSourceInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CreateCatalogSyncSourceInput")
            .field("source_id", &self.source_id)
            .field("artist_id", &self.artist_id)
            .field("kind", &self.kind)
            .field("locator", &"[REDACTED]")
            .field("storefront", &self.storefront)
            .field("locale", &self.locale)
            .finish()
    }
}

impl CreateCatalogSyncSourceInput {
    fn into_command(self) -> NewManagedCatalogSource {
        NewManagedCatalogSource {
            source_id: self.source_id,
            artist_id: self.artist_id,
            kind: self.kind.into(),
            locator: self.locator,
            storefront: self.storefront,
            locale: self.locale,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, InputObject)]
pub struct StartCatalogSyncRunInput {
    run_id: Option<Uuid>,
    source_id: Uuid,
}

impl StartCatalogSyncRunInput {
    fn into_command(self) -> NewCatalogSyncRun {
        NewCatalogSyncRun {
            run_id: self.run_id,
            source_id: self.source_id,
            requested_cursor: None,
        }
    }
}

pub async fn query_source(
    ctx: &Context<'_>,
    source_id: Uuid,
) -> Result<Option<CatalogSyncSourceInfo>> {
    ctx.data::<CatalogSyncService>()?
        .get_source(source_id)
        .await
        .map(|value| value.map(Into::into))
        .map_err(catalog_sync_error)
}

pub async fn query_sources(
    ctx: &Context<'_>,
    artist_id: Uuid,
) -> Result<Vec<CatalogSyncSourceInfo>> {
    ctx.data::<CatalogSyncService>()?
        .list_sources_for_artist(artist_id)
        .await
        .map(|sources| sources.into_iter().map(Into::into).collect())
        .map_err(catalog_sync_error)
}

pub async fn query_run(ctx: &Context<'_>, run_id: Uuid) -> Result<Option<CatalogSyncRunInfo>> {
    ctx.data::<CatalogSyncService>()?
        .get_run(run_id)
        .await
        .map(|value| value.map(Into::into))
        .map_err(catalog_sync_error)
}

pub async fn create_source(
    ctx: &Context<'_>,
    input: CreateCatalogSyncSourceInput,
) -> Result<CatalogSyncSourceInfo> {
    ctx.data::<CatalogSyncService>()?
        .create_managed_source(input.into_command())
        .await
        .map(Into::into)
        .map_err(catalog_sync_error)
}

pub async fn start_run(
    ctx: &Context<'_>,
    input: StartCatalogSyncRunInput,
) -> Result<CatalogSyncRunInfo> {
    ctx.data::<CatalogSyncService>()?
        .start_managed_run(input.into_command())
        .await
        .map(Into::into)
        .map_err(catalog_sync_error)
}

fn catalog_sync_error(error: CatalogSyncError) -> Error {
    match error {
        CatalogSyncError::ArtistNotFound { artist_id } => {
            entity_error("CATALOG_SYNC_ARTIST_NOT_FOUND", "artistId", artist_id)
        }
        CatalogSyncError::SourceAlreadyExists { source_id } => {
            entity_error("CATALOG_SYNC_SOURCE_ALREADY_EXISTS", "sourceId", source_id)
        }
        CatalogSyncError::SourceIdentityAlreadyExists { artist_id, kind } => {
            Error::new("catalog source identity already exists").extend_with(|_, extensions| {
                extensions.set("code", "CATALOG_SYNC_SOURCE_IDENTITY_EXISTS");
                extensions.set("artistId", artist_id.to_string());
                extensions.set("kind", kind.as_str());
            })
        }
        CatalogSyncError::SourceNotFound { source_id } => {
            entity_error("CATALOG_SYNC_SOURCE_NOT_FOUND", "sourceId", source_id)
        }
        CatalogSyncError::SourceDisabled { source_id } => {
            entity_error("CATALOG_SYNC_SOURCE_DISABLED", "sourceId", source_id)
        }
        CatalogSyncError::CredentialNotConfigured { kind } => Error::new(
            "catalog sync credential is not configured on this server",
        )
        .extend_with(|_, extensions| {
            extensions.set("code", "CATALOG_SYNC_CREDENTIAL_NOT_CONFIGURED");
            extensions.set("kind", kind.as_str());
        }),
        CatalogSyncError::CredentialBindingInvalid { kind } => {
            Error::new("catalog sync credential binding is invalid").extend_with(|_, extensions| {
                extensions.set("code", "CATALOG_SYNC_CREDENTIAL_BINDING_INVALID");
                extensions.set("kind", kind.as_str());
            })
        }
        CatalogSyncError::AdapterUnavailable { kind } => Error::new(
            "catalog sync adapter is not available in this deployment",
        )
        .extend_with(|_, extensions| {
            extensions.set("code", "CATALOG_SYNC_ADAPTER_UNAVAILABLE");
            extensions.set("kind", kind.as_str());
        }),
        CatalogSyncError::SourceBusy { source_id } => {
            entity_error("CATALOG_SYNC_SOURCE_BUSY", "sourceId", source_id)
        }
        CatalogSyncError::RunAlreadyExists { run_id } => {
            entity_error("CATALOG_SYNC_RUN_ALREADY_EXISTS", "runId", run_id)
        }
        CatalogSyncError::RunNotFound { run_id } => {
            entity_error("CATALOG_SYNC_RUN_NOT_FOUND", "runId", run_id)
        }
        CatalogSyncError::RunConflict {
            run_id,
            expected,
            actual,
        } => Error::new("catalog sync run changed concurrently").extend_with(|_, extensions| {
            extensions.set("code", "CATALOG_SYNC_RUN_CONFLICT");
            extensions.set("runId", run_id.to_string());
            extensions.set("expectedRowVersion", expected.to_string());
            extensions.set("actualRowVersion", actual.to_string());
        }),
        CatalogSyncError::InvalidRunTransition { run_id, from, to } => Error::new(
            "catalog sync run cannot make that transition",
        )
        .extend_with(|_, extensions| {
            extensions.set("code", "CATALOG_SYNC_INVALID_TRANSITION");
            extensions.set("runId", run_id.to_string());
            extensions.set("from", from.as_str());
            extensions.set("to", to.as_str());
        }),
        CatalogSyncError::RunNotRunning { run_id, status } => {
            Error::new("catalog sync run is not running").extend_with(|_, extensions| {
                extensions.set("code", "CATALOG_SYNC_RUN_NOT_RUNNING");
                extensions.set("runId", run_id.to_string());
                extensions.set("status", status.as_str());
            })
        }
        CatalogSyncError::InvalidInput { field, message } => {
            Error::new(message).extend_with(|_, extensions| {
                extensions.set("code", "CATALOG_SYNC_INVALID_INPUT");
                extensions.set("field", field);
            })
        }
        CatalogSyncError::RunNotClaimable { .. }
        | CatalogSyncError::LeaseMismatch { .. }
        | CatalogSyncError::ObservationConflict { .. }
        | CatalogSyncError::InvalidPersistedValue { .. }
        | CatalogSyncError::MissingObservationRevision { .. }
        | CatalogSyncError::InvalidObservationDigestLength { .. }
        | CatalogSyncError::ObservationDigestMismatch { .. }
        | CatalogSyncError::NumericOutOfRange { .. }
        | CatalogSyncError::Database(_) => {
            tracing::error!("catalog sync operation failed internally");
            Error::new("catalog sync operation failed")
                .extend_with(|_, extensions| extensions.set("code", "CATALOG_SYNC_INTERNAL"))
        }
    }
}

fn entity_error(code: &'static str, id_field: &'static str, id: Uuid) -> Error {
    Error::new(code).extend_with(|_, extensions| {
        extensions.set("code", code);
        extensions.set(id_field, id.to_string());
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::CatalogRowVersion;

    #[test]
    fn browser_snapshots_drop_private_source_and_run_fields_losslessly() {
        let timestamp = DateTime::<Utc>::from_timestamp(1_700_000_000, 0).unwrap();
        let locator = "https://example.invalid/releases?token=source-secret".to_owned();
        let source = CatalogSyncSourceInfo::from(CatalogSourceSnapshot {
            source_id: Uuid::new_v4(),
            artist_id: Uuid::new_v4(),
            kind: DomainCatalogSourceKind::ArtistWebsite,
            locator: locator.clone(),
            storefront: Some("jp".to_owned()),
            locale: Some("ja-JP".to_owned()),
            enabled: true,
            provisioning_state: DomainCatalogSourceProvisioningState::AdapterUnavailable,
            row_version: CatalogRowVersion::new(u64::MAX).unwrap(),
            created_at: timestamp,
            updated_at: timestamp,
        });
        let source_debug = format!("{source:?}");
        assert_eq!(source.row_version, u64::MAX.to_string());
        assert_eq!(
            source.provisioning_state,
            CatalogSyncProvisioningState::AdapterUnavailable
        );
        assert!(!source_debug.contains(&locator));
        assert!(!source_debug.contains("token=source-secret"));

        let run = CatalogSyncRunInfo::from(CatalogSyncRunSnapshot {
            run_id: Uuid::new_v4(),
            source_id: source.source_id,
            status: DomainSyncRunStatus::Running,
            requested_cursor: Some("requested-secret".to_owned()),
            result_cursor: Some("result-secret".to_owned()),
            observed_count: u32::MAX,
            error_message: Some("adapter-secret".to_owned()),
            coverage: DomainSyncCoverage::Incremental,
            started_from_root: false,
            snapshot_complete: true,
            attempt_count: u32::MAX,
            next_attempt_at: Some(timestamp),
            row_version: CatalogRowVersion::new(u64::MAX).unwrap(),
            created_at: timestamp,
            started_at: Some(timestamp),
            finished_at: None,
        });
        let run_debug = format!("{run:?}");
        assert_eq!(run.observed_count, u32::MAX.to_string());
        assert_eq!(run.attempt_count, u32::MAX.to_string());
        assert_eq!(run.coverage, CatalogSyncCoverage::Incremental);
        assert_eq!(run.row_version, u64::MAX.to_string());
        assert!(!run_debug.contains("requested-secret"));
        assert!(!run_debug.contains("result-secret"));
        assert!(!run_debug.contains("adapter-secret"));
    }

    #[test]
    fn browser_write_inputs_redact_locators_and_build_managed_commands() {
        let locator = "https://example.invalid/api?signature=secret".to_owned();
        let input = CreateCatalogSyncSourceInput {
            source_id: None,
            artist_id: Uuid::new_v4(),
            kind: CatalogSyncSourceKind::RecordLabel,
            locator: locator.clone(),
            storefront: None,
            locale: None,
        };
        let debug = format!("{input:?}");
        assert!(!debug.contains(&locator));
        assert!(!debug.contains("signature=secret"));
        assert!(debug.contains("[REDACTED]"));

        let command = input.into_command();
        assert_eq!(command.locator, locator);
        assert_eq!(command.kind, DomainCatalogSourceKind::RecordLabel);

        let run = StartCatalogSyncRunInput {
            run_id: None,
            source_id: Uuid::new_v4(),
        }
        .into_command();
        assert!(run.requested_cursor.is_none());
    }
}
