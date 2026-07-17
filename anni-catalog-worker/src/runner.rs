use std::{collections::HashSet, fmt, future::Future, num::NonZeroU32, pin::Pin, time::Duration};

use anni_catalog::SyncCoverage;
use annim::catalog::{
    CatalogReleaseObservation, CatalogRowVersion, CatalogSyncError, CatalogSyncLease,
    CatalogSyncService, FinishCatalogSyncRun,
};
use chrono::{DateTime, Utc};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    AdapterFailure, AdapterFailureDisposition, AdapterObservation, AdapterPage, CatalogAdapters,
};

type QueueFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, CatalogSyncError>> + Send + 'a>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatalogWorkerPolicy {
    pub lease_for: Duration,
    pub page_timeout: Duration,
    pub base_retry_delay: Duration,
    pub max_retry_delay: Duration,
    pub max_attempts: NonZeroU32,
    pub max_pages: NonZeroU32,
    pub max_observations_per_page: NonZeroU32,
    pub max_total_observations: NonZeroU32,
    pub max_reference_bytes: NonZeroU32,
    pub max_document_bytes: NonZeroU32,
    pub max_page_bytes: NonZeroU32,
}

impl Default for CatalogWorkerPolicy {
    fn default() -> Self {
        Self {
            lease_for: Duration::from_secs(10 * 60),
            page_timeout: Duration::from_secs(60),
            base_retry_delay: Duration::from_secs(30),
            max_retry_delay: Duration::from_secs(6 * 60 * 60),
            max_attempts: NonZeroU32::new(5).expect("five is non-zero"),
            max_pages: NonZeroU32::new(100).expect("one hundred is non-zero"),
            max_observations_per_page: NonZeroU32::new(500).expect("five hundred is non-zero"),
            max_total_observations: NonZeroU32::new(10_000).expect("ten thousand is non-zero"),
            max_reference_bytes: NonZeroU32::new(16 * 1024).expect("sixteen KiB is non-zero"),
            max_document_bytes: NonZeroU32::new(8 * 1024 * 1024).expect("eight MiB is non-zero"),
            max_page_bytes: NonZeroU32::new(32 * 1024 * 1024).expect("thirty-two MiB is non-zero"),
        }
    }
}

impl CatalogWorkerPolicy {
    fn validate(self) -> Result<Self, CatalogWorkerError> {
        if self.lease_for.is_zero() {
            return Err(CatalogWorkerError::InvalidPolicy {
                field: "lease_for",
                message: "duration must be positive",
            });
        }
        if self.page_timeout.is_zero() {
            return Err(CatalogWorkerError::InvalidPolicy {
                field: "page_timeout",
                message: "duration must be positive",
            });
        }
        if self.page_timeout >= self.lease_for {
            return Err(CatalogWorkerError::InvalidPolicy {
                field: "page_timeout",
                message: "duration must be shorter than lease_for",
            });
        }
        if self.base_retry_delay.is_zero() {
            return Err(CatalogWorkerError::InvalidPolicy {
                field: "base_retry_delay",
                message: "duration must be positive",
            });
        }
        if self.max_retry_delay < self.base_retry_delay {
            return Err(CatalogWorkerError::InvalidPolicy {
                field: "max_retry_delay",
                message: "duration must not be shorter than base_retry_delay",
            });
        }
        chrono::Duration::from_std(self.max_retry_delay).map_err(|_| {
            CatalogWorkerError::InvalidPolicy {
                field: "max_retry_delay",
                message: "duration cannot be represented as a timestamp delta",
            }
        })?;
        if self.max_page_bytes < self.max_document_bytes {
            return Err(CatalogWorkerError::InvalidPolicy {
                field: "max_page_bytes",
                message: "limit must not be smaller than max_document_bytes",
            });
        }
        if self.max_page_bytes < self.max_reference_bytes {
            return Err(CatalogWorkerError::InvalidPolicy {
                field: "max_page_bytes",
                message: "limit must not be smaller than max_reference_bytes",
            });
        }
        Ok(self)
    }

    fn retry_delay(self, attempt_count: u32, requested: Option<Duration>) -> Duration {
        let exponent = attempt_count.saturating_sub(1).min(31);
        let multiplier = 1_u32.checked_shl(exponent).unwrap_or(u32::MAX);
        requested
            .unwrap_or_else(|| self.base_retry_delay.saturating_mul(multiplier))
            .min(self.max_retry_delay)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogFailureReason {
    PermanentFailure,
    AttemptsExhausted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogWorkerOutcome {
    Idle,
    Succeeded {
        run_id: Uuid,
        coverage: SyncCoverage,
        processed_count: u32,
    },
    RetryScheduled {
        run_id: Uuid,
        attempt_count: u32,
        not_before: DateTime<Utc>,
        failure: AdapterFailure,
    },
    Failed {
        run_id: Uuid,
        attempt_count: u32,
        reason: CatalogFailureReason,
        failure: AdapterFailure,
    },
}

#[derive(Error)]
pub enum CatalogWorkerError {
    #[error("invalid catalog worker policy {field}: {message}")]
    InvalidPolicy {
        field: &'static str,
        message: &'static str,
    },
    #[error("catalog queue operation failed")]
    Queue(#[from] CatalogSyncError),
    #[error("catalog retry deadline exceeds the supported timestamp range")]
    RetryDeadlineOverflow,
}

impl fmt::Debug for CatalogWorkerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPolicy { field, message } => formatter
                .debug_struct("InvalidPolicy")
                .field("field", field)
                .field("message", message)
                .finish(),
            Self::Queue(_) => formatter.write_str("Queue([REDACTED])"),
            Self::RetryDeadlineOverflow => formatter.write_str("RetryDeadlineOverflow"),
        }
    }
}

pub struct CatalogWorker<A> {
    core: CatalogWorkerCore<CatalogSyncService, A>,
}

impl<A: CatalogAdapters> CatalogWorker<A> {
    pub fn new(
        queue: CatalogSyncService,
        adapters: A,
        policy: CatalogWorkerPolicy,
    ) -> Result<Self, CatalogWorkerError> {
        Ok(Self {
            core: CatalogWorkerCore::new(queue, adapters, policy.validate()?),
        })
    }

    pub async fn run_once(&self) -> Result<CatalogWorkerOutcome, CatalogWorkerError> {
        self.core.run_once_at(Utc::now()).await
    }
}

trait CatalogQueue: Send + Sync {
    fn claim_next<'a>(&'a self, lease_for: Duration) -> QueueFuture<'a, Option<CatalogSyncLease>>;

    fn renew<'a>(
        &'a self,
        lease: &'a CatalogSyncLease,
        lease_for: Duration,
    ) -> QueueFuture<'a, CatalogSyncLease>;

    fn record<'a>(
        &'a self,
        lease: &'a CatalogSyncLease,
        observation: AdapterObservation,
    ) -> QueueFuture<'a, CatalogRowVersion>;

    fn retry<'a>(
        &'a self,
        lease: &'a CatalogSyncLease,
        failure_code: &'static str,
        resume_cursor: Option<String>,
        not_before: DateTime<Utc>,
    ) -> QueueFuture<'a, ()>;

    fn finish<'a>(
        &'a self,
        lease: &'a CatalogSyncLease,
        outcome: FinishCatalogSyncRun,
    ) -> QueueFuture<'a, ()>;
}

impl CatalogQueue for CatalogSyncService {
    fn claim_next<'a>(&'a self, lease_for: Duration) -> QueueFuture<'a, Option<CatalogSyncLease>> {
        Box::pin(async move { CatalogSyncService::claim_next(self, lease_for).await })
    }

    fn renew<'a>(
        &'a self,
        lease: &'a CatalogSyncLease,
        lease_for: Duration,
    ) -> QueueFuture<'a, CatalogSyncLease> {
        Box::pin(async move {
            CatalogSyncService::renew_lease(
                self,
                lease.run_id,
                lease.row_version,
                lease.lease_token,
                lease_for,
            )
            .await
        })
    }

    fn record<'a>(
        &'a self,
        lease: &'a CatalogSyncLease,
        observation: AdapterObservation,
    ) -> QueueFuture<'a, CatalogRowVersion> {
        Box::pin(async move {
            CatalogSyncService::record_observation(
                self,
                lease.run_id,
                lease.row_version,
                lease.lease_token,
                CatalogReleaseObservation {
                    external_release_id: observation.external_release_id,
                    source_url: observation.source_url,
                    raw_document: observation.raw_document,
                    parsed_document: observation.parsed_document,
                },
            )
            .await
            .map(|recorded| recorded.run.row_version)
        })
    }

    fn retry<'a>(
        &'a self,
        lease: &'a CatalogSyncLease,
        failure_code: &'static str,
        resume_cursor: Option<String>,
        not_before: DateTime<Utc>,
    ) -> QueueFuture<'a, ()> {
        Box::pin(async move {
            CatalogSyncService::retry_run(
                self,
                lease.run_id,
                lease.row_version,
                lease.lease_token,
                failure_code,
                resume_cursor,
                not_before,
            )
            .await
            .map(|_| ())
        })
    }

    fn finish<'a>(
        &'a self,
        lease: &'a CatalogSyncLease,
        outcome: FinishCatalogSyncRun,
    ) -> QueueFuture<'a, ()> {
        Box::pin(async move {
            CatalogSyncService::finish_run(
                self,
                lease.run_id,
                lease.row_version,
                lease.lease_token,
                outcome,
            )
            .await
            .map(|_| ())
        })
    }
}

struct CatalogWorkerCore<Q, A> {
    queue: Q,
    adapters: A,
    policy: CatalogWorkerPolicy,
}

impl<Q: CatalogQueue, A: CatalogAdapters> CatalogWorkerCore<Q, A> {
    const fn new(queue: Q, adapters: A, policy: CatalogWorkerPolicy) -> Self {
        Self {
            queue,
            adapters,
            policy,
        }
    }

    async fn run_once_at(
        &self,
        now: DateTime<Utc>,
    ) -> Result<CatalogWorkerOutcome, CatalogWorkerError> {
        let Some(mut lease) = self.queue.claim_next(self.policy.lease_for).await? else {
            return Ok(CatalogWorkerOutcome::Idle);
        };
        let mut cursor = lease.requested_cursor.clone();
        if cursor
            .as_ref()
            .is_some_and(|cursor| !reference_fits(cursor, self.policy))
        {
            return self
                .handle_failure(
                    lease,
                    None,
                    AdapterFailure::permanent("cursor_too_large", None),
                    now,
                )
                .await;
        }
        let mut seen_cursors = HashSet::new();
        if let Some(cursor) = cursor.as_ref() {
            seen_cursors.insert(cursor.clone());
        }
        let mut declared_coverage = None;
        let mut checkpoint = None;
        let mut page_count = 0_u32;
        let mut observed_count = 0_u32;

        loop {
            if page_count >= self.policy.max_pages.get() {
                return self
                    .handle_failure(
                        lease,
                        cursor,
                        AdapterFailure::permanent("page_limit_exceeded", None),
                        now,
                    )
                    .await;
            }

            lease = self.queue.renew(&lease, self.policy.lease_for).await?;
            let page = match tokio::time::timeout(
                self.policy.page_timeout,
                self.adapters.fetch_page(&lease, cursor.as_deref()),
            )
            .await
            {
                Ok(Ok(page)) => page,
                Ok(Err(failure)) => {
                    return self.handle_failure(lease, cursor, failure, now).await;
                }
                Err(_) => {
                    return self
                        .handle_failure(
                            lease,
                            cursor,
                            AdapterFailure::retryable("adapter_timeout", None, None),
                            now,
                        )
                        .await;
                }
            };
            // Network work may consume most of a lease. Renew before writing
            // the page so every observation is owned by a current token.
            lease = self.queue.renew(&lease, self.policy.lease_for).await?;

            if let Err(failure) = validate_page(&page, declared_coverage, self.policy) {
                return self.handle_failure(lease, cursor, failure, now).await;
            }
            declared_coverage.get_or_insert(page.coverage);
            page_count = page_count.saturating_add(1);

            for observation in page.observations {
                if let Err(failure) = validate_observation(&observation, self.policy) {
                    return self.handle_failure(lease, cursor, failure, now).await;
                }
                observed_count =
                    observed_count
                        .checked_add(1)
                        .ok_or(CatalogWorkerError::InvalidPolicy {
                            field: "max_total_observations",
                            message: "counter overflow",
                        })?;
                if observed_count > self.policy.max_total_observations.get() {
                    return self
                        .handle_failure(
                            lease,
                            cursor,
                            AdapterFailure::permanent("observation_limit_exceeded", None),
                            now,
                        )
                        .await;
                }
                lease.row_version = self.queue.record(&lease, observation).await?;
            }

            if page.checkpoint.is_some() {
                checkpoint = page.checkpoint;
            }
            if page.complete {
                let coverage = declared_coverage.unwrap_or(SyncCoverage::DiscoveryOnly);
                if coverage == SyncCoverage::FullSnapshot
                    && observed_count == 0
                    && !page.empty_full_snapshot_confirmed
                {
                    return self
                        .handle_failure(
                            lease,
                            cursor,
                            AdapterFailure::permanent("empty_full_snapshot_not_confirmed", None),
                            now,
                        )
                        .await;
                }
                self.queue
                    .finish(
                        &lease,
                        FinishCatalogSyncRun::Succeeded {
                            result_cursor: checkpoint,
                            coverage,
                            snapshot_complete: true,
                        },
                    )
                    .await?;
                return Ok(CatalogWorkerOutcome::Succeeded {
                    run_id: lease.run_id,
                    coverage,
                    processed_count: observed_count,
                });
            }

            let next_cursor = page
                .next_cursor
                .expect("validated non-complete pages always have a cursor");
            if !seen_cursors.insert(next_cursor.clone()) {
                return self
                    .handle_failure(
                        lease,
                        cursor,
                        AdapterFailure::permanent("cursor_cycle", None),
                        now,
                    )
                    .await;
            }
            cursor = Some(next_cursor);
        }
    }

    async fn handle_failure(
        &self,
        lease: CatalogSyncLease,
        resume_cursor: Option<String>,
        failure: AdapterFailure,
        now: DateTime<Utc>,
    ) -> Result<CatalogWorkerOutcome, CatalogWorkerError> {
        if failure.is_retryable() && lease.attempt_count < self.policy.max_attempts.get() {
            let delay = self
                .policy
                .retry_delay(lease.attempt_count, failure.retry_after());
            let delta = chrono::Duration::from_std(delay)
                .map_err(|_| CatalogWorkerError::RetryDeadlineOverflow)?;
            let not_before = now
                .checked_add_signed(delta)
                .ok_or(CatalogWorkerError::RetryDeadlineOverflow)?;
            self.queue
                .retry(&lease, failure.code(), resume_cursor, not_before)
                .await?;
            return Ok(CatalogWorkerOutcome::RetryScheduled {
                run_id: lease.run_id,
                attempt_count: lease.attempt_count,
                not_before,
                failure,
            });
        }

        let reason = if failure.disposition() == AdapterFailureDisposition::Retryable {
            CatalogFailureReason::AttemptsExhausted
        } else {
            CatalogFailureReason::PermanentFailure
        };
        self.queue
            .finish(
                &lease,
                FinishCatalogSyncRun::Failed {
                    error_message: failure.code().to_owned(),
                },
            )
            .await?;
        Ok(CatalogWorkerOutcome::Failed {
            run_id: lease.run_id,
            attempt_count: lease.attempt_count,
            reason,
            failure,
        })
    }
}

fn validate_page(
    page: &AdapterPage,
    previous_coverage: Option<SyncCoverage>,
    policy: CatalogWorkerPolicy,
) -> Result<(), AdapterFailure> {
    if page.observations.len() > policy.max_observations_per_page.get() as usize {
        return Err(AdapterFailure::permanent(
            "page_observation_limit_exceeded",
            None,
        ));
    }
    if previous_coverage.is_some_and(|coverage| coverage != page.coverage) {
        return Err(AdapterFailure::permanent("coverage_changed", None));
    }
    if page.complete == page.next_cursor.is_some() {
        return Err(AdapterFailure::permanent("invalid_pagination_state", None));
    }
    if page
        .next_cursor
        .as_ref()
        .is_some_and(|cursor| !reference_fits(cursor, policy))
        || page
            .checkpoint
            .as_ref()
            .is_some_and(|checkpoint| !reference_fits(checkpoint, policy))
    {
        return Err(AdapterFailure::permanent("cursor_too_large", None));
    }
    let page_bytes = page.observations.iter().try_fold(
        page.next_cursor.as_ref().map_or(0, String::len)
            + page.checkpoint.as_ref().map_or(0, String::len),
        |total, observation| {
            total
                .checked_add(observation_bytes(observation))
                .ok_or_else(|| AdapterFailure::permanent("page_size_overflow", None))
        },
    )?;
    if page_bytes > policy.max_page_bytes.get() as usize {
        return Err(AdapterFailure::permanent("page_too_large", None));
    }
    Ok(())
}

fn validate_observation(
    observation: &AdapterObservation,
    policy: CatalogWorkerPolicy,
) -> Result<(), AdapterFailure> {
    if observation.external_release_id.is_empty()
        || observation.source_url.is_empty()
        || observation.raw_document.is_empty()
        || observation.parsed_document.is_empty()
    {
        return Err(AdapterFailure::permanent("invalid_observation", None));
    }
    if !reference_fits(&observation.external_release_id, policy)
        || !reference_fits(&observation.source_url, policy)
    {
        return Err(AdapterFailure::permanent(
            "observation_reference_too_large",
            None,
        ));
    }
    if observation.raw_document.len() > policy.max_document_bytes.get() as usize
        || observation.parsed_document.len() > policy.max_document_bytes.get() as usize
    {
        return Err(AdapterFailure::permanent(
            "observation_document_too_large",
            None,
        ));
    }
    Ok(())
}

fn reference_fits(value: &str, policy: CatalogWorkerPolicy) -> bool {
    value.len() <= policy.max_reference_bytes.get() as usize
}

fn observation_bytes(observation: &AdapterObservation) -> usize {
    observation
        .external_release_id
        .len()
        .saturating_add(observation.source_url.len())
        .saturating_add(observation.raw_document.len())
        .saturating_add(observation.parsed_document.len())
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, sync::Mutex};

    use anni_catalog::CatalogSourceKind;

    use super::*;
    use crate::{AdapterFuture, CatalogAdapters};

    struct FakeAdapters {
        pages: Mutex<VecDeque<Result<AdapterPage, AdapterFailure>>>,
    }

    impl FakeAdapters {
        fn new(pages: impl IntoIterator<Item = Result<AdapterPage, AdapterFailure>>) -> Self {
            Self {
                pages: Mutex::new(pages.into_iter().collect()),
            }
        }
    }

    impl CatalogAdapters for FakeAdapters {
        fn fetch_page<'a>(
            &'a self,
            _lease: &'a CatalogSyncLease,
            _cursor: Option<&'a str>,
        ) -> AdapterFuture<'a> {
            let page = self.pages.lock().unwrap().pop_front().unwrap();
            Box::pin(async move { page })
        }
    }

    struct PendingAdapters;

    impl CatalogAdapters for PendingAdapters {
        fn fetch_page<'a>(
            &'a self,
            _lease: &'a CatalogSyncLease,
            _cursor: Option<&'a str>,
        ) -> AdapterFuture<'a> {
            Box::pin(std::future::pending())
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum QueueAction {
        Observation(AdapterObservation),
        Retry {
            code: &'static str,
            cursor: Option<String>,
            not_before: DateTime<Utc>,
        },
        Finish(FinishCatalogSyncRun),
    }

    struct FakeQueue {
        lease: Mutex<Option<CatalogSyncLease>>,
        current: Mutex<CatalogSyncLease>,
        actions: Mutex<Vec<QueueAction>>,
    }

    impl FakeQueue {
        fn new(lease: CatalogSyncLease) -> Self {
            Self {
                lease: Mutex::new(Some(lease.clone())),
                current: Mutex::new(lease),
                actions: Mutex::new(Vec::new()),
            }
        }

        fn actions(&self) -> Vec<QueueAction> {
            self.actions.lock().unwrap().clone()
        }

        fn advance(&self) -> CatalogSyncLease {
            let mut lease = self.current.lock().unwrap();
            lease.row_version = CatalogRowVersion::new(lease.row_version.get() + 1).unwrap();
            lease.clone()
        }
    }

    impl CatalogQueue for FakeQueue {
        fn claim_next<'a>(
            &'a self,
            _lease_for: Duration,
        ) -> QueueFuture<'a, Option<CatalogSyncLease>> {
            let lease = self.lease.lock().unwrap().take();
            Box::pin(async move { Ok(lease) })
        }

        fn renew<'a>(
            &'a self,
            _lease: &'a CatalogSyncLease,
            _lease_for: Duration,
        ) -> QueueFuture<'a, CatalogSyncLease> {
            let lease = self.advance();
            Box::pin(async move { Ok(lease) })
        }

        fn record<'a>(
            &'a self,
            _lease: &'a CatalogSyncLease,
            observation: AdapterObservation,
        ) -> QueueFuture<'a, CatalogRowVersion> {
            self.actions
                .lock()
                .unwrap()
                .push(QueueAction::Observation(observation));
            let version = self.advance().row_version;
            Box::pin(async move { Ok(version) })
        }

        fn retry<'a>(
            &'a self,
            _lease: &'a CatalogSyncLease,
            failure_code: &'static str,
            resume_cursor: Option<String>,
            not_before: DateTime<Utc>,
        ) -> QueueFuture<'a, ()> {
            self.actions.lock().unwrap().push(QueueAction::Retry {
                code: failure_code,
                cursor: resume_cursor,
                not_before,
            });
            Box::pin(async { Ok(()) })
        }

        fn finish<'a>(
            &'a self,
            _lease: &'a CatalogSyncLease,
            outcome: FinishCatalogSyncRun,
        ) -> QueueFuture<'a, ()> {
            self.actions
                .lock()
                .unwrap()
                .push(QueueAction::Finish(outcome));
            Box::pin(async { Ok(()) })
        }
    }

    fn lease(attempt_count: u32) -> CatalogSyncLease {
        CatalogSyncLease {
            run_id: Uuid::new_v4(),
            source_id: Uuid::new_v4(),
            kind: CatalogSourceKind::ArtistWebsite,
            locator: "https://artist.example/discography?token=must-not-leak".to_owned(),
            storefront: None,
            locale: Some("ja-JP".to_owned()),
            configuration_document: None,
            secret_ref: Some("secret/catalog/artist".to_owned()),
            requested_cursor: None,
            lease_token: Uuid::new_v4(),
            lease_expires_at: Utc::now() + chrono::Duration::minutes(10),
            attempt_count,
            row_version: CatalogRowVersion::INITIAL,
        }
    }

    fn observation(id: &str) -> AdapterObservation {
        AdapterObservation {
            external_release_id: id.to_owned(),
            source_url: format!("https://artist.example/{id}?signature=must-not-leak"),
            raw_document: format!(r#"{{"title":"作品・A〜B～C（初回）","id":"{id}"}}"#),
            parsed_document: format!(
                r#"{{"schemaVersion":1,"title":"作品・A〜B～C（初回）","id":"{id}"}}"#
            ),
        }
    }

    fn policy() -> CatalogWorkerPolicy {
        CatalogWorkerPolicy {
            lease_for: Duration::from_secs(60),
            page_timeout: Duration::from_millis(20),
            base_retry_delay: Duration::from_secs(10),
            max_retry_delay: Duration::from_secs(40),
            max_attempts: NonZeroU32::new(3).unwrap(),
            max_pages: NonZeroU32::new(4).unwrap(),
            max_observations_per_page: NonZeroU32::new(5).unwrap(),
            max_total_observations: NonZeroU32::new(10).unwrap(),
            max_reference_bytes: NonZeroU32::new(128).unwrap(),
            max_document_bytes: NonZeroU32::new(1024).unwrap(),
            max_page_bytes: NonZeroU32::new(4096).unwrap(),
        }
    }

    #[tokio::test]
    async fn complete_full_snapshot_records_exact_pages_then_succeeds() {
        let queue = FakeQueue::new(lease(1));
        let first = observation("album/100");
        let second = observation("album/200");
        let adapters = FakeAdapters::new([
            Ok(AdapterPage {
                observations: vec![first.clone()],
                next_cursor: Some("page:2-secret".to_owned()),
                checkpoint: None,
                coverage: SyncCoverage::FullSnapshot,
                complete: false,
                empty_full_snapshot_confirmed: false,
            }),
            Ok(AdapterPage {
                observations: vec![second.clone()],
                next_cursor: None,
                checkpoint: Some("next-run-secret".to_owned()),
                coverage: SyncCoverage::FullSnapshot,
                complete: true,
                empty_full_snapshot_confirmed: false,
            }),
        ]);
        let worker = CatalogWorkerCore::new(queue, adapters, policy());

        let outcome = worker.run_once_at(Utc::now()).await.unwrap();

        assert!(matches!(
            outcome,
            CatalogWorkerOutcome::Succeeded {
                coverage: SyncCoverage::FullSnapshot,
                processed_count: 2,
                ..
            }
        ));
        let actions = worker.queue.actions();
        assert_eq!(actions[0], QueueAction::Observation(first));
        assert_eq!(actions[1], QueueAction::Observation(second));
        assert!(matches!(
            &actions[2],
            QueueAction::Finish(FinishCatalogSyncRun::Succeeded {
                result_cursor: Some(cursor),
                coverage: SyncCoverage::FullSnapshot,
                snapshot_complete: true,
            }) if cursor == "next-run-secret"
        ));
        assert!(!format!("{outcome:?}").contains("must-not-leak"));
    }

    #[tokio::test]
    async fn retryable_page_failure_preserves_the_private_resume_cursor() {
        let lease = lease(1);
        let run_id = lease.run_id;
        let queue = FakeQueue::new(lease);
        let failure = AdapterFailure::retryable("upstream_unavailable", Some(503), None);
        let adapters = FakeAdapters::new([
            Ok(AdapterPage {
                observations: vec![observation("album/100")],
                next_cursor: Some("page:2-secret".to_owned()),
                checkpoint: None,
                coverage: SyncCoverage::DiscoveryOnly,
                complete: false,
                empty_full_snapshot_confirmed: false,
            }),
            Err(failure),
        ]);
        let worker = CatalogWorkerCore::new(queue, adapters, policy());
        let now = DateTime::parse_from_rfc3339("2026-07-12T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let outcome = worker.run_once_at(now).await.unwrap();

        assert_eq!(
            outcome,
            CatalogWorkerOutcome::RetryScheduled {
                run_id,
                attempt_count: 1,
                not_before: now + chrono::Duration::seconds(10),
                failure,
            }
        );
        assert!(matches!(
            worker.queue.actions().last(),
            Some(QueueAction::Retry {
                code: "upstream_unavailable",
                cursor: Some(cursor),
                ..
            }) if cursor == "page:2-secret"
        ));
        assert!(!format!("{outcome:?}").contains("page:2-secret"));
    }

    #[tokio::test]
    async fn malformed_pagination_and_exhausted_retries_finish_safely() {
        let invalid_code = AdapterFailure::retryable("token secret", None, None);
        assert_eq!(invalid_code.code(), "invalid_adapter_failure_code");
        assert_eq!(
            invalid_code.disposition(),
            AdapterFailureDisposition::Permanent
        );
        assert!(!format!("{invalid_code:?}").contains("token secret"));

        let queue = FakeQueue::new(lease(1));
        let adapters = FakeAdapters::new([Ok(AdapterPage {
            observations: vec![],
            next_cursor: None,
            checkpoint: None,
            coverage: SyncCoverage::FullSnapshot,
            complete: false,
            empty_full_snapshot_confirmed: false,
        })]);
        let worker = CatalogWorkerCore::new(queue, adapters, policy());
        let malformed = worker.run_once_at(Utc::now()).await.unwrap();
        assert!(matches!(
            malformed,
            CatalogWorkerOutcome::Failed {
                reason: CatalogFailureReason::PermanentFailure,
                failure,
                ..
            } if failure.code() == "invalid_pagination_state"
        ));

        let queue = FakeQueue::new(lease(policy().max_attempts.get()));
        let failure = AdapterFailure::retryable("rate_limited", Some(429), None);
        let adapters = FakeAdapters::new([Err(failure)]);
        let worker = CatalogWorkerCore::new(queue, adapters, policy());
        let exhausted = worker.run_once_at(Utc::now()).await.unwrap();
        assert!(matches!(
            exhausted,
            CatalogWorkerOutcome::Failed {
                reason: CatalogFailureReason::AttemptsExhausted,
                failure: actual,
                ..
            } if actual == failure
        ));
    }

    #[tokio::test]
    async fn empty_full_snapshot_requires_an_explicit_confirmation() {
        let queue = FakeQueue::new(lease(1));
        let adapters = FakeAdapters::new([Ok(AdapterPage {
            observations: vec![],
            next_cursor: None,
            checkpoint: None,
            coverage: SyncCoverage::FullSnapshot,
            complete: true,
            empty_full_snapshot_confirmed: false,
        })]);
        let worker = CatalogWorkerCore::new(queue, adapters, policy());
        let rejected = worker.run_once_at(Utc::now()).await.unwrap();
        assert!(matches!(
            rejected,
            CatalogWorkerOutcome::Failed { failure, .. }
                if failure.code() == "empty_full_snapshot_not_confirmed"
        ));

        let queue = FakeQueue::new(lease(1));
        let adapters = FakeAdapters::new([Ok(AdapterPage {
            observations: vec![],
            next_cursor: None,
            checkpoint: None,
            coverage: SyncCoverage::FullSnapshot,
            complete: true,
            empty_full_snapshot_confirmed: true,
        })]);
        let worker = CatalogWorkerCore::new(queue, adapters, policy());
        let accepted = worker.run_once_at(Utc::now()).await.unwrap();
        assert!(matches!(
            accepted,
            CatalogWorkerOutcome::Succeeded {
                coverage: SyncCoverage::FullSnapshot,
                processed_count: 0,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn adapter_documents_are_bounded_before_persistence() {
        let mut at_limit = observation("album/100");
        at_limit.raw_document = "x".repeat(policy().max_document_bytes.get() as usize);
        let queue = FakeQueue::new(lease(1));
        let adapters = FakeAdapters::new([Ok(AdapterPage {
            observations: vec![at_limit],
            next_cursor: None,
            checkpoint: None,
            coverage: SyncCoverage::DiscoveryOnly,
            complete: true,
            empty_full_snapshot_confirmed: false,
        })]);
        let worker = CatalogWorkerCore::new(queue, adapters, policy());
        assert!(matches!(
            worker.run_once_at(Utc::now()).await.unwrap(),
            CatalogWorkerOutcome::Succeeded {
                processed_count: 1,
                ..
            }
        ));

        let mut oversized = observation("album/100");
        oversized.raw_document = "x".repeat(policy().max_document_bytes.get() as usize + 1);
        let queue = FakeQueue::new(lease(1));
        let adapters = FakeAdapters::new([Ok(AdapterPage {
            observations: vec![oversized],
            next_cursor: None,
            checkpoint: None,
            coverage: SyncCoverage::DiscoveryOnly,
            complete: true,
            empty_full_snapshot_confirmed: false,
        })]);
        let worker = CatalogWorkerCore::new(queue, adapters, policy());
        let rejected = worker.run_once_at(Utc::now()).await.unwrap();
        assert!(matches!(
            rejected,
            CatalogWorkerOutcome::Failed { failure, .. }
                if failure.code() == "observation_document_too_large"
        ));
    }

    #[tokio::test]
    async fn hung_adapter_page_is_cancelled_and_retried() {
        let queue = FakeQueue::new(lease(1));
        let worker = CatalogWorkerCore::new(queue, PendingAdapters, policy());
        let now = DateTime::parse_from_rfc3339("2026-07-12T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let outcome = worker.run_once_at(now).await.unwrap();

        assert!(matches!(
            outcome,
            CatalogWorkerOutcome::RetryScheduled { failure, .. }
                if failure.code() == "adapter_timeout"
        ));
    }
}
