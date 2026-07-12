//! Background orchestration for verified cover downloads.
//!
//! [`CoverService`] owns durable queue state while [`anni_cover_worker`]
//! owns all network and filesystem access.  This module is the deliberately
//! small seam between them: it claims one lease, fetches one image, then
//! records exactly one terminal action for that lease.

use std::{
    future::Future,
    num::{NonZeroU32, NonZeroU64},
    pin::Pin,
    time::Duration,
};

use anni_cover_worker::{
    AssetStore, CoverDownloader, CoverWorkerError as DownloadError, VerifiedCoverDownload,
};
use chrono::{DateTime, Utc};
use sea_orm::prelude::Uuid;
use thiserror::Error;

use crate::cover::{
    CoverError, CoverFetchFailure, CoverFetchLease, CoverRowVersion, CoverService,
    VerifiedCoverAsset,
};

pub type CoverFetchFuture<'a> =
    Pin<Box<dyn Future<Output = Result<VerifiedCoverAsset, CoverFetchProblem>> + Send + 'a>>;
type QueueFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, CoverError>> + Send + 'a>>;

/// A replaceable acquisition boundary.
///
/// Production uses [`SafeCoverFetcher`]. Tests can supply an in-memory
/// implementation without opening sockets or writing files.
pub trait CoverFetcher: Send + Sync {
    fn fetch<'a>(&'a self, request_url: &'a str) -> CoverFetchFuture<'a>;
}

/// Production fetcher backed by the SSRF-resistant downloader and a trusted
/// content-addressed asset store.
#[derive(Debug, Clone)]
pub struct SafeCoverFetcher {
    downloader: CoverDownloader,
    asset_store: AssetStore,
}

impl SafeCoverFetcher {
    pub const fn new(downloader: CoverDownloader, asset_store: AssetStore) -> Self {
        Self {
            downloader,
            asset_store,
        }
    }
}

impl CoverFetcher for SafeCoverFetcher {
    fn fetch<'a>(&'a self, request_url: &'a str) -> CoverFetchFuture<'a> {
        Box::pin(async move {
            let download = self
                .downloader
                .download(request_url, &self.asset_store)
                .await
                .map_err(CoverFetchProblem::from)?;
            verified_download_to_asset(download)
        })
    }
}

/// Whether a failure can reasonably change when attempted again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverFailureDisposition {
    Permanent,
    Retryable,
}

/// A bounded, persistence-safe description of a fetch failure.
///
/// It intentionally contains neither a URL nor an arbitrary error message.
/// The downloader error is reduced to a stable code before it crosses into
/// queue persistence or worker diagnostics.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct CoverFetchProblem {
    code: &'static str,
    disposition: CoverFailureDisposition,
    http_status: Option<u16>,
}

impl CoverFetchProblem {
    pub const fn code(self) -> &'static str {
        self.code
    }

    pub const fn disposition(self) -> CoverFailureDisposition {
        self.disposition
    }

    pub const fn http_status(self) -> Option<u16> {
        self.http_status
    }

    const fn permanent(code: &'static str, http_status: Option<u16>) -> Self {
        Self {
            code,
            disposition: CoverFailureDisposition::Permanent,
            http_status,
        }
    }

    const fn retryable(code: &'static str, http_status: Option<u16>) -> Self {
        Self {
            code,
            disposition: CoverFailureDisposition::Retryable,
            http_status,
        }
    }

    const fn is_retryable(self) -> bool {
        matches!(self.disposition, CoverFailureDisposition::Retryable)
    }

    fn persisted(self) -> CoverFetchFailure {
        CoverFetchFailure {
            code: self.code.to_owned(),
            message: None,
            http_status: self.http_status,
        }
    }
}

impl std::fmt::Debug for CoverFetchProblem {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CoverFetchProblem")
            .field("code", &self.code)
            .field("disposition", &self.disposition)
            .field("http_status", &self.http_status)
            .finish()
    }
}

impl From<DownloadError> for CoverFetchProblem {
    fn from(error: DownloadError) -> Self {
        match error {
            DownloadError::InvalidUrl(_)
            | DownloadError::HttpsRequired
            | DownloadError::MissingHost
            | DownloadError::EmbeddedCredentials => Self::permanent("invalid_url", None),
            DownloadError::ForbiddenAddress { .. } => Self::permanent("forbidden_address", None),
            DownloadError::Resolve { .. } | DownloadError::NoAddresses { .. } => {
                Self::retryable("dns_failure", None)
            }
            DownloadError::Client(_) => Self::retryable("http_client_failure", None),
            DownloadError::Request { .. } => Self::retryable("request_failure", None),
            DownloadError::HttpStatus { status, .. } => {
                if is_retryable_http_status(status) {
                    Self::retryable("http_status", Some(status))
                } else {
                    Self::permanent("http_status", Some(status))
                }
            }
            DownloadError::RedirectWithoutLocation
            | DownloadError::InvalidRedirectLocation
            | DownloadError::TooManyRedirects => Self::permanent("invalid_redirect", None),
            DownloadError::BodyTooLarge { .. } | DownloadError::ImageTooLarge => {
                Self::permanent("image_too_large", None)
            }
            DownloadError::EmptyBody => Self::retryable("empty_body", None),
            DownloadError::UnsupportedImageFormat => Self::permanent("unsupported_image", None),
            DownloadError::Image(_) => Self::permanent("invalid_image", None),
            DownloadError::InvalidStorageKey(_) => Self::permanent("invalid_storage_key", None),
            // Retrying either condition would repeatedly touch an integrity
            // boundary with the same immutable bytes/path. An operator must
            // inspect and explicitly requeue the candidate after remediation.
            DownloadError::DigestCollision { .. } => Self::permanent("digest_collision", None),
            DownloadError::AssetPathEscapesRoot { .. } => {
                Self::permanent("asset_path_rejected", None)
            }
            DownloadError::AssetRootNotDirectory { .. } => {
                Self::permanent("invalid_asset_root", None)
            }
            DownloadError::VerifierPanicked => Self::retryable("verifier_failure", None),
            DownloadError::Io(_) => Self::retryable("io_failure", None),
        }
    }
}

fn is_retryable_http_status(status: u16) -> bool {
    matches!(status, 408 | 425 | 429) || (500..=599).contains(&status)
}

fn verified_download_to_asset(
    download: VerifiedCoverDownload,
) -> Result<VerifiedCoverAsset, CoverFetchProblem> {
    let width = NonZeroU32::new(download.width())
        .ok_or_else(|| CoverFetchProblem::permanent("invalid_image", None))?;
    let height = NonZeroU32::new(download.height())
        .ok_or_else(|| CoverFetchProblem::permanent("invalid_image", None))?;
    NonZeroU64::new(download.byte_length())
        .ok_or_else(|| CoverFetchProblem::permanent("empty_body", None))?;

    Ok(VerifiedCoverAsset {
        content_sha256: download.digest(),
        media_type: download.media_type(),
        width,
        height,
        byte_length: download.byte_length(),
        effective_url: Some(download.effective_url().to_owned()),
    })
}

/// Worker timing and retry limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoverWorkerPolicy {
    pub lease_for: Duration,
    pub base_retry_delay: Duration,
    pub max_retry_delay: Duration,
    pub max_attempts: NonZeroU32,
}

impl CoverWorkerPolicy {
    fn validate(self) -> Result<Self, CoverWorkerRunError> {
        if self.lease_for.is_zero() {
            return Err(CoverWorkerRunError::InvalidPolicy {
                field: "lease_for",
                message: "duration must be positive",
            });
        }
        if self.base_retry_delay.is_zero() {
            return Err(CoverWorkerRunError::InvalidPolicy {
                field: "base_retry_delay",
                message: "duration must be positive",
            });
        }
        if self.max_retry_delay < self.base_retry_delay {
            return Err(CoverWorkerRunError::InvalidPolicy {
                field: "max_retry_delay",
                message: "duration must not be shorter than base_retry_delay",
            });
        }
        chrono::Duration::from_std(self.max_retry_delay).map_err(|_| {
            CoverWorkerRunError::InvalidPolicy {
                field: "max_retry_delay",
                message: "duration cannot be represented as a timestamp delta",
            }
        })?;
        Ok(self)
    }

    fn retry_delay(self, attempt_count: u32) -> Duration {
        let exponent = attempt_count.saturating_sub(1).min(31);
        let multiplier = 1_u32.checked_shl(exponent).unwrap_or(u32::MAX);
        self.base_retry_delay
            .saturating_mul(multiplier)
            .min(self.max_retry_delay)
    }
}

impl Default for CoverWorkerPolicy {
    fn default() -> Self {
        Self {
            // One download can follow several separately timed redirects, so
            // the lease is intentionally longer than a single HTTP timeout.
            lease_for: Duration::from_secs(10 * 60),
            base_retry_delay: Duration::from_secs(30),
            max_retry_delay: Duration::from_secs(6 * 60 * 60),
            max_attempts: NonZeroU32::new(5).expect("five is non-zero"),
        }
    }
}

/// Why a failed candidate was moved to the rejected state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverRejectionReason {
    PermanentFailure,
    AttemptsExhausted,
}

/// Result of processing at most one queue item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoverWorkerOutcome {
    Idle,
    Verified {
        candidate_id: Uuid,
        attempt_count: u32,
    },
    RetryScheduled {
        candidate_id: Uuid,
        attempt_count: u32,
        not_before: DateTime<Utc>,
        problem: CoverFetchProblem,
    },
    Rejected {
        candidate_id: Uuid,
        attempt_count: u32,
        reason: CoverRejectionReason,
        problem: CoverFetchProblem,
    },
}

#[derive(Debug, Error)]
pub enum CoverWorkerRunError {
    #[error("invalid cover worker policy {field}: {message}")]
    InvalidPolicy {
        field: &'static str,
        message: &'static str,
    },
    #[error("cover queue operation failed")]
    Queue(#[from] CoverError),
    #[error("cover retry deadline exceeds the supported timestamp range")]
    RetryDeadlineOverflow,
}

/// Processes queued cover candidates one at a time.
pub struct CoverWorker<F> {
    core: CoverWorkerCore<CoverService, F>,
}

impl<F> CoverWorker<F>
where
    F: CoverFetcher,
{
    pub fn new(
        covers: CoverService,
        fetcher: F,
        policy: CoverWorkerPolicy,
    ) -> Result<Self, CoverWorkerRunError> {
        Ok(Self {
            core: CoverWorkerCore::new(covers, fetcher, policy.validate()?),
        })
    }

    /// Claims and processes no more than one candidate.
    pub async fn run_once(&self) -> Result<CoverWorkerOutcome, CoverWorkerRunError> {
        self.core.run_once_at(Utc::now()).await
    }
}

trait CoverQueue: Send + Sync {
    fn claim_next<'a>(&'a self, lease_for: Duration) -> QueueFuture<'a, Option<CoverFetchLease>>;

    fn complete_fetch<'a>(
        &'a self,
        candidate_id: Uuid,
        expected: CoverRowVersion,
        lease_token: Uuid,
        verified: VerifiedCoverAsset,
    ) -> QueueFuture<'a, ()>;

    fn retry_fetch<'a>(
        &'a self,
        candidate_id: Uuid,
        expected: CoverRowVersion,
        lease_token: Uuid,
        failure: CoverFetchFailure,
        not_before: DateTime<Utc>,
    ) -> QueueFuture<'a, ()>;

    fn reject_fetch<'a>(
        &'a self,
        candidate_id: Uuid,
        expected: CoverRowVersion,
        lease_token: Uuid,
        failure: CoverFetchFailure,
    ) -> QueueFuture<'a, ()>;
}

impl CoverQueue for CoverService {
    fn claim_next<'a>(&'a self, lease_for: Duration) -> QueueFuture<'a, Option<CoverFetchLease>> {
        Box::pin(async move { CoverService::claim_next(self, lease_for).await })
    }

    fn complete_fetch<'a>(
        &'a self,
        candidate_id: Uuid,
        expected: CoverRowVersion,
        lease_token: Uuid,
        verified: VerifiedCoverAsset,
    ) -> QueueFuture<'a, ()> {
        Box::pin(async move {
            CoverService::complete_fetch(self, candidate_id, expected, lease_token, verified)
                .await
                .map(|_| ())
        })
    }

    fn retry_fetch<'a>(
        &'a self,
        candidate_id: Uuid,
        expected: CoverRowVersion,
        lease_token: Uuid,
        failure: CoverFetchFailure,
        not_before: DateTime<Utc>,
    ) -> QueueFuture<'a, ()> {
        Box::pin(async move {
            CoverService::retry_fetch(
                self,
                candidate_id,
                expected,
                lease_token,
                failure,
                not_before,
            )
            .await
            .map(|_| ())
        })
    }

    fn reject_fetch<'a>(
        &'a self,
        candidate_id: Uuid,
        expected: CoverRowVersion,
        lease_token: Uuid,
        failure: CoverFetchFailure,
    ) -> QueueFuture<'a, ()> {
        Box::pin(async move {
            CoverService::reject_fetch(self, candidate_id, expected, lease_token, failure)
                .await
                .map(|_| ())
        })
    }
}

struct CoverWorkerCore<Q, F> {
    covers: Q,
    fetcher: F,
    policy: CoverWorkerPolicy,
}

impl<Q, F> CoverWorkerCore<Q, F>
where
    Q: CoverQueue,
    F: CoverFetcher,
{
    const fn new(covers: Q, fetcher: F, policy: CoverWorkerPolicy) -> Self {
        Self {
            covers,
            fetcher,
            policy,
        }
    }

    async fn run_once_at(
        &self,
        now: DateTime<Utc>,
    ) -> Result<CoverWorkerOutcome, CoverWorkerRunError> {
        let Some(lease) = self.covers.claim_next(self.policy.lease_for).await? else {
            return Ok(CoverWorkerOutcome::Idle);
        };

        match self.fetcher.fetch(&lease.request_url).await {
            Ok(verified) => {
                self.covers
                    .complete_fetch(
                        lease.candidate_id,
                        lease.row_version,
                        lease.lease_token,
                        verified,
                    )
                    .await?;
                Ok(CoverWorkerOutcome::Verified {
                    candidate_id: lease.candidate_id,
                    attempt_count: lease.attempt_count,
                })
            }
            Err(problem)
                if problem.is_retryable()
                    && lease.attempt_count < self.policy.max_attempts.get() =>
            {
                let retry_delta =
                    chrono::Duration::from_std(self.policy.retry_delay(lease.attempt_count))
                        .map_err(|_| CoverWorkerRunError::RetryDeadlineOverflow)?;
                let not_before = now
                    .checked_add_signed(retry_delta)
                    .ok_or(CoverWorkerRunError::RetryDeadlineOverflow)?;
                self.covers
                    .retry_fetch(
                        lease.candidate_id,
                        lease.row_version,
                        lease.lease_token,
                        problem.persisted(),
                        not_before,
                    )
                    .await?;
                Ok(CoverWorkerOutcome::RetryScheduled {
                    candidate_id: lease.candidate_id,
                    attempt_count: lease.attempt_count,
                    not_before,
                    problem,
                })
            }
            Err(problem) => {
                let reason = if problem.is_retryable() {
                    CoverRejectionReason::AttemptsExhausted
                } else {
                    CoverRejectionReason::PermanentFailure
                };
                self.covers
                    .reject_fetch(
                        lease.candidate_id,
                        lease.row_version,
                        lease.lease_token,
                        problem.persisted(),
                    )
                    .await?;
                Ok(CoverWorkerOutcome::Rejected {
                    candidate_id: lease.candidate_id,
                    attempt_count: lease.attempt_count,
                    reason,
                    problem,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use anni_catalog::{CoverMediaType, CoverSourceKind};
    use anni_ingest::Digest;

    use super::*;

    #[derive(Debug, Clone)]
    struct FakeFetcher {
        result: Result<VerifiedCoverAsset, CoverFetchProblem>,
    }

    impl CoverFetcher for FakeFetcher {
        fn fetch<'a>(&'a self, _request_url: &'a str) -> CoverFetchFuture<'a> {
            let result = self.result.clone();
            Box::pin(async move { result })
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum QueueAction {
        Completed(VerifiedCoverAsset),
        Retried(CoverFetchFailure, DateTime<Utc>),
        Rejected(CoverFetchFailure),
    }

    #[derive(Debug)]
    struct FakeQueue {
        lease: Mutex<Option<CoverFetchLease>>,
        actions: Mutex<Vec<QueueAction>>,
    }

    impl FakeQueue {
        fn new(lease: CoverFetchLease) -> Self {
            Self {
                lease: Mutex::new(Some(lease)),
                actions: Mutex::new(Vec::new()),
            }
        }

        fn actions(&self) -> Vec<QueueAction> {
            self.actions.lock().unwrap().clone()
        }
    }

    impl CoverQueue for FakeQueue {
        fn claim_next<'a>(
            &'a self,
            _lease_for: Duration,
        ) -> QueueFuture<'a, Option<CoverFetchLease>> {
            let lease = self.lease.lock().unwrap().take();
            Box::pin(async move { Ok(lease) })
        }

        fn complete_fetch<'a>(
            &'a self,
            _candidate_id: Uuid,
            _expected: CoverRowVersion,
            _lease_token: Uuid,
            verified: VerifiedCoverAsset,
        ) -> QueueFuture<'a, ()> {
            self.actions
                .lock()
                .unwrap()
                .push(QueueAction::Completed(verified));
            Box::pin(async { Ok(()) })
        }

        fn retry_fetch<'a>(
            &'a self,
            _candidate_id: Uuid,
            _expected: CoverRowVersion,
            _lease_token: Uuid,
            failure: CoverFetchFailure,
            not_before: DateTime<Utc>,
        ) -> QueueFuture<'a, ()> {
            self.actions
                .lock()
                .unwrap()
                .push(QueueAction::Retried(failure, not_before));
            Box::pin(async { Ok(()) })
        }

        fn reject_fetch<'a>(
            &'a self,
            _candidate_id: Uuid,
            _expected: CoverRowVersion,
            _lease_token: Uuid,
            failure: CoverFetchFailure,
        ) -> QueueFuture<'a, ()> {
            self.actions
                .lock()
                .unwrap()
                .push(QueueAction::Rejected(failure));
            Box::pin(async { Ok(()) })
        }
    }

    fn lease(attempt_count: u32) -> CoverFetchLease {
        CoverFetchLease {
            candidate_id: Uuid::new_v4(),
            release_id: Uuid::new_v4(),
            disc_number: 0,
            source_kind: CoverSourceKind::ArtistWebsite,
            request_url: "https://artist.example/cover.jpg?token=must-not-leak".to_owned(),
            lease_token: Uuid::new_v4(),
            lease_expires_at: Utc::now() + chrono::Duration::minutes(10),
            attempt_count,
            row_version: CoverRowVersion::INITIAL,
        }
    }

    fn asset() -> VerifiedCoverAsset {
        VerifiedCoverAsset {
            content_sha256: Digest::new([0x42; Digest::LENGTH]),
            media_type: CoverMediaType::Png,
            width: NonZeroU32::new(3_000).unwrap(),
            height: NonZeroU32::new(3_000).unwrap(),
            byte_length: 9_000_000,
            effective_url: Some("https://cdn.example/final.png?signature=must-not-leak".to_owned()),
        }
    }

    fn test_policy() -> CoverWorkerPolicy {
        CoverWorkerPolicy {
            lease_for: Duration::from_secs(60),
            base_retry_delay: Duration::from_secs(10),
            max_retry_delay: Duration::from_secs(40),
            max_attempts: NonZeroU32::new(3).unwrap(),
        }
    }

    #[tokio::test]
    async fn successful_fetch_completes_the_claimed_lease() {
        let lease = lease(1);
        let candidate_id = lease.candidate_id;
        let queue = FakeQueue::new(lease);
        let expected_asset = asset();
        let worker = CoverWorkerCore::new(
            queue,
            FakeFetcher {
                result: Ok(expected_asset.clone()),
            },
            test_policy(),
        );

        let outcome = worker.run_once_at(Utc::now()).await.unwrap();

        assert_eq!(
            outcome,
            CoverWorkerOutcome::Verified {
                candidate_id,
                attempt_count: 1
            }
        );
        assert_eq!(
            worker.covers.actions(),
            vec![QueueAction::Completed(expected_asset)]
        );
        assert!(!format!("{outcome:?}").contains("must-not-leak"));
    }

    #[tokio::test]
    async fn retryable_failure_uses_bounded_backoff_without_persisting_a_message() {
        let lease = lease(3);
        let candidate_id = lease.candidate_id;
        let queue = FakeQueue::new(lease);
        let worker = CoverWorkerCore::new(
            queue,
            FakeFetcher {
                result: Err(CoverFetchProblem::retryable("request_failure", None)),
            },
            CoverWorkerPolicy {
                max_attempts: NonZeroU32::new(4).unwrap(),
                ..test_policy()
            },
        );
        let now = DateTime::parse_from_rfc3339("2026-07-12T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let outcome = worker.run_once_at(now).await.unwrap();

        let expected_at = now + chrono::Duration::seconds(40);
        assert_eq!(
            outcome,
            CoverWorkerOutcome::RetryScheduled {
                candidate_id,
                attempt_count: 3,
                not_before: expected_at,
                problem: CoverFetchProblem::retryable("request_failure", None),
            }
        );
        assert_eq!(
            worker.covers.actions(),
            vec![QueueAction::Retried(
                CoverFetchFailure {
                    code: "request_failure".to_owned(),
                    message: None,
                    http_status: None,
                },
                expected_at,
            )]
        );
    }

    #[tokio::test]
    async fn permanent_failure_rejects_immediately_with_only_a_safe_code() {
        let lease = lease(1);
        let candidate_id = lease.candidate_id;
        let queue = FakeQueue::new(lease);
        let problem = CoverFetchProblem::from(DownloadError::DigestCollision {
            storage_key: "sha256/42/42/collision.png".to_owned(),
        });
        let worker = CoverWorkerCore::new(
            queue,
            FakeFetcher {
                result: Err(problem),
            },
            test_policy(),
        );

        let outcome = worker.run_once_at(Utc::now()).await.unwrap();

        assert_eq!(
            outcome,
            CoverWorkerOutcome::Rejected {
                candidate_id,
                attempt_count: 1,
                reason: CoverRejectionReason::PermanentFailure,
                problem,
            }
        );
        assert_eq!(
            worker.covers.actions(),
            vec![QueueAction::Rejected(CoverFetchFailure {
                code: "digest_collision".to_owned(),
                message: None,
                http_status: None,
            })]
        );
    }
}
