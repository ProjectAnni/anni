//! Apple Music catalog synchronization through Apple's documented API.
//!
//! An Annim source maps `locator` to the numeric Apple artist ID,
//! `storefront` to the two-letter catalog storefront, `locale` to the optional
//! localization tag, and `secret_ref` to a developer token in the worker's
//! secret store. The endpoint itself is not configurable: every request stays
//! under the artist's `/albums` relationship on `api.music.apple.com`.
//!
//! Each album resource is retained as the exact JSON fragment received from
//! Apple. The parsed document adds a schema-versioned envelope without
//! normalizing any string values.

use std::{
    collections::HashSet,
    fmt,
    future::Future,
    pin::Pin,
    sync::Arc,
    time::{Duration, SystemTime},
};

use anni_catalog::{CatalogSourceKind, SyncCoverage};
use annim::catalog::CatalogSyncLease;
use futures_util::StreamExt;
use reqwest::{
    header::{ACCEPT, CONTENT_TYPE, RETRY_AFTER},
    redirect::Policy,
};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use thiserror::Error;
use url::Url;

use crate::{AdapterFailure, AdapterFuture, AdapterObservation, AdapterPage, CatalogAdapter};

const APPLE_MUSIC_ORIGIN: &str = "https://api.music.apple.com";
const PAGE_LIMIT: u16 = 100;
const MAX_CURSOR_BYTES: usize = 4 * 1024;
const MAX_SECRET_REF_BYTES: usize = 512;
const MAX_DEVELOPER_TOKEN_BYTES: usize = 16 * 1024;

pub type SecretFuture<'a> =
    Pin<Box<dyn Future<Output = Result<SecretValue, AdapterFailure>> + Send + 'a>>;

/// Resolves a private value by reference inside the worker process.
///
/// Annim persists only `secret_ref`; the developer token never enters the
/// catalog database or the GraphQL API.
pub trait SecretResolver: Send + Sync + 'static {
    fn resolve<'a>(&'a self, secret_ref: &'a str) -> SecretFuture<'a>;
}

/// An Apple Music developer token with redacted formatting.
#[derive(Clone, PartialEq, Eq)]
pub struct SecretValue(String);

impl SecretValue {
    pub fn new(value: impl Into<String>) -> Result<Self, SecretValueError> {
        let value = value.into();
        validate_developer_token(&value)?;
        Ok(Self(value))
    }

    /// Exposes the secret only to a transport that is about to construct the
    /// Authorization header.
    pub fn expose_secret(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretValue([REDACTED])")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum SecretValueError {
    #[error("the Apple Music developer token is empty")]
    Empty,
    #[error("the Apple Music developer token exceeds the supported size")]
    TooLong,
    #[error("the Apple Music developer token is not a compact JWT")]
    InvalidCompactJwt,
}

fn validate_developer_token(value: &str) -> Result<(), SecretValueError> {
    if value.is_empty() {
        return Err(SecretValueError::Empty);
    }
    if value.len() > MAX_DEVELOPER_TOKEN_BYTES {
        return Err(SecretValueError::TooLong);
    }
    let mut segments = value.split('.');
    let valid_segment = |segment: &str| {
        !segment.is_empty()
            && segment
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    };
    let compact = match (
        segments.next(),
        segments.next(),
        segments.next(),
        segments.next(),
    ) {
        (Some(header), Some(claims), Some(signature), None) => {
            valid_segment(header) && valid_segment(claims) && valid_segment(signature)
        }
        _ => false,
    };
    if !compact {
        return Err(SecretValueError::InvalidCompactJwt);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppleMusicHttpPolicy {
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
    pub max_response_bytes: u64,
}

impl Default for AppleMusicHttpPolicy {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(30),
            max_response_bytes: 4 * 1024 * 1024,
        }
    }
}

impl AppleMusicHttpPolicy {
    fn validate(self) -> Result<Self, AppleMusicHttpBuildError> {
        if self.connect_timeout.is_zero() {
            return Err(AppleMusicHttpBuildError::InvalidPolicy {
                field: "connect_timeout",
                message: "duration must be positive",
            });
        }
        if self.request_timeout.is_zero() {
            return Err(AppleMusicHttpBuildError::InvalidPolicy {
                field: "request_timeout",
                message: "duration must be positive",
            });
        }
        if self.connect_timeout > self.request_timeout {
            return Err(AppleMusicHttpBuildError::InvalidPolicy {
                field: "connect_timeout",
                message: "duration must not exceed request_timeout",
            });
        }
        if self.max_response_bytes == 0 {
            return Err(AppleMusicHttpBuildError::InvalidPolicy {
                field: "max_response_bytes",
                message: "limit must be positive",
            });
        }
        Ok(self)
    }
}

#[derive(Debug, Error)]
pub enum AppleMusicHttpBuildError {
    #[error("invalid Apple Music HTTP policy {field}: {message}")]
    InvalidPolicy {
        field: &'static str,
        message: &'static str,
    },
    #[error("failed to construct the Apple Music HTTP client")]
    Client(#[source] reqwest::Error),
}

#[derive(Clone)]
pub struct AppleMusicHttpRequest {
    url: Url,
    developer_token: SecretValue,
}

impl AppleMusicHttpRequest {
    pub fn url(&self) -> &Url {
        &self.url
    }

    pub fn developer_token(&self) -> &SecretValue {
        &self.developer_token
    }
}

impl fmt::Debug for AppleMusicHttpRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AppleMusicHttpRequest")
            .field("origin", &APPLE_MUSIC_ORIGIN)
            .field("path", &self.url.path())
            .field("has_query", &self.url.query().is_some())
            .field("developer_token", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AppleMusicHttpResponse {
    status: u16,
    body: String,
    content_type_is_json: bool,
    retry_after: Option<Duration>,
}

impl AppleMusicHttpResponse {
    pub fn json(status: u16, body: impl Into<String>, retry_after: Option<Duration>) -> Self {
        Self {
            status,
            body: body.into(),
            content_type_is_json: true,
            retry_after,
        }
    }

    pub fn non_json(status: u16, body: impl Into<String>) -> Self {
        Self {
            status,
            body: body.into(),
            content_type_is_json: false,
            retry_after: None,
        }
    }
}

impl fmt::Debug for AppleMusicHttpResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AppleMusicHttpResponse")
            .field("status", &self.status)
            .field("body_bytes", &self.body.len())
            .field("content_type_is_json", &self.content_type_is_json)
            .field("retry_after", &self.retry_after)
            .finish()
    }
}

pub type AppleMusicTransportFuture<'a> =
    Pin<Box<dyn Future<Output = Result<AppleMusicHttpResponse, AdapterFailure>> + Send + 'a>>;

pub trait AppleMusicTransport: Send + Sync + 'static {
    fn get<'a>(&'a self, request: AppleMusicHttpRequest) -> AppleMusicTransportFuture<'a>;
}

/// Production transport for the fixed Apple Music API origin.
#[derive(Clone)]
pub struct AppleMusicHttpTransport {
    client: reqwest::Client,
    policy: AppleMusicHttpPolicy,
}

impl AppleMusicHttpTransport {
    pub fn new(policy: AppleMusicHttpPolicy) -> Result<Self, AppleMusicHttpBuildError> {
        let policy = policy.validate()?;
        let client = reqwest::Client::builder()
            .redirect(Policy::none())
            .no_proxy()
            .referer(false)
            .connect_timeout(policy.connect_timeout)
            .timeout(policy.request_timeout)
            .user_agent("ProjectAnni-CatalogWorker/0.1")
            .build()
            .map_err(AppleMusicHttpBuildError::Client)?;
        Ok(Self { client, policy })
    }

    pub const fn policy(&self) -> AppleMusicHttpPolicy {
        self.policy
    }

    async fn get_inner(
        &self,
        request: AppleMusicHttpRequest,
    ) -> Result<AppleMusicHttpResponse, AdapterFailure> {
        validate_transport_url(&request.url)?;
        let response = self
            .client
            .get(request.url)
            .header(ACCEPT, "application/json")
            .bearer_auth(request.developer_token.expose_secret())
            .send()
            .await
            .map_err(classify_request_error)?;
        let status = response.status().as_u16();
        let retry_after = response
            .headers()
            .get(RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| parse_retry_after(value, SystemTime::now()));
        let content_type_is_json = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(is_json_content_type);
        if response
            .content_length()
            .is_some_and(|length| length > self.policy.max_response_bytes)
        {
            return Err(AdapterFailure::permanent(
                "apple_response_too_large",
                Some(status),
            ));
        }

        let mut body = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| {
                AdapterFailure::retryable("apple_response_interrupted", Some(status), None)
            })?;
            let next_len = body.len().checked_add(chunk.len()).ok_or_else(|| {
                AdapterFailure::permanent("apple_response_too_large", Some(status))
            })?;
            if next_len as u64 > self.policy.max_response_bytes {
                return Err(AdapterFailure::permanent(
                    "apple_response_too_large",
                    Some(status),
                ));
            }
            body.extend_from_slice(&chunk);
        }
        let body = String::from_utf8(body)
            .map_err(|_| AdapterFailure::permanent("apple_response_not_utf8", Some(status)))?;
        Ok(AppleMusicHttpResponse {
            status,
            body,
            content_type_is_json,
            retry_after,
        })
    }
}

impl fmt::Debug for AppleMusicHttpTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AppleMusicHttpTransport")
            .field("origin", &APPLE_MUSIC_ORIGIN)
            .field("policy", &self.policy)
            .finish()
    }
}

impl AppleMusicTransport for AppleMusicHttpTransport {
    fn get<'a>(&'a self, request: AppleMusicHttpRequest) -> AppleMusicTransportFuture<'a> {
        Box::pin(async move { self.get_inner(request).await })
    }
}

fn classify_request_error(error: reqwest::Error) -> AdapterFailure {
    if error.is_timeout() {
        AdapterFailure::retryable("apple_request_timeout", None, None)
    } else {
        AdapterFailure::retryable("apple_request_failed", None, None)
    }
}

fn is_json_content_type(value: &str) -> bool {
    value
        .split(';')
        .next()
        .is_some_and(|mime| mime.trim().eq_ignore_ascii_case("application/json"))
}

fn parse_retry_after(value: &str, now: SystemTime) -> Option<Duration> {
    if let Ok(seconds) = value.trim().parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }
    let deadline = httpdate::parse_http_date(value).ok()?;
    deadline.duration_since(now).ok().or(Some(Duration::ZERO))
}

fn validate_transport_url(url: &Url) -> Result<(), AdapterFailure> {
    if url.scheme() != "https"
        || url.host_str() != Some("api.music.apple.com")
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(AdapterFailure::permanent(
            "apple_request_origin_invalid",
            None,
        ));
    }
    Ok(())
}

pub struct AppleMusicAdapter<R, T = AppleMusicHttpTransport> {
    secrets: Arc<R>,
    transport: T,
}

impl<R: SecretResolver> AppleMusicAdapter<R, AppleMusicHttpTransport> {
    pub fn new(
        secrets: Arc<R>,
        policy: AppleMusicHttpPolicy,
    ) -> Result<Self, AppleMusicHttpBuildError> {
        Ok(Self {
            secrets,
            transport: AppleMusicHttpTransport::new(policy)?,
        })
    }
}

impl<R, T> AppleMusicAdapter<R, T> {
    pub fn with_transport(secrets: Arc<R>, transport: T) -> Self {
        Self { secrets, transport }
    }
}

impl<R, T> fmt::Debug for AppleMusicAdapter<R, T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AppleMusicAdapter")
            .field("origin", &APPLE_MUSIC_ORIGIN)
            .field("secret_resolver", &"[REDACTED]")
            .field("transport", &std::any::type_name::<T>())
            .finish()
    }
}

impl<R: SecretResolver, T: AppleMusicTransport> AppleMusicAdapter<R, T> {
    async fn fetch_page_inner(
        &self,
        lease: &CatalogSyncLease,
        cursor: Option<&str>,
    ) -> Result<AdapterPage, AdapterFailure> {
        let scope = AppleMusicScope::from_lease(lease)?;
        let request_url = scope.request_url(cursor)?;
        let secret_ref = validate_secret_ref(lease.secret_ref.as_deref())?;
        let developer_token = self.secrets.resolve(secret_ref).await?;
        let response = self
            .transport
            .get(AppleMusicHttpRequest {
                url: request_url,
                developer_token,
            })
            .await?;
        if response.status != 200 {
            return Err(failure_for_status(response.status, response.retry_after));
        }
        if !response.content_type_is_json {
            return Err(AdapterFailure::permanent(
                "apple_response_not_json",
                Some(response.status),
            ));
        }
        parse_page(&scope, cursor, &response.body)
    }
}

impl<R: SecretResolver, T: AppleMusicTransport> CatalogAdapter for AppleMusicAdapter<R, T> {
    fn source_kind(&self) -> CatalogSourceKind {
        CatalogSourceKind::AppleMusic
    }

    fn fetch_page<'a>(
        &'a self,
        lease: &'a CatalogSyncLease,
        cursor: Option<&'a str>,
    ) -> AdapterFuture<'a> {
        Box::pin(async move { self.fetch_page_inner(lease, cursor).await })
    }
}

fn failure_for_status(status: u16, retry_after: Option<Duration>) -> AdapterFailure {
    match status {
        401 | 403 => AdapterFailure::permanent("apple_authorization_failed", Some(status)),
        404 => AdapterFailure::permanent("apple_artist_not_found", Some(status)),
        429 => AdapterFailure::retryable("apple_rate_limited", Some(status), retry_after),
        408 | 425 | 500..=599 => {
            AdapterFailure::retryable("apple_upstream_unavailable", Some(status), retry_after)
        }
        300..=399 => AdapterFailure::permanent("apple_redirect_rejected", Some(status)),
        100..=299 | 400..=499 => AdapterFailure::permanent("apple_request_rejected", Some(status)),
        _ => AdapterFailure::permanent("apple_invalid_http_status", None),
    }
}

#[derive(Clone)]
struct AppleMusicScope {
    storefront: String,
    artist_id: String,
    locale: Option<String>,
    albums_path: String,
}

impl AppleMusicScope {
    fn from_lease(lease: &CatalogSyncLease) -> Result<Self, AdapterFailure> {
        if lease.kind != CatalogSourceKind::AppleMusic {
            return Err(AdapterFailure::permanent("apple_source_kind_invalid", None));
        }
        if lease.configuration_document.is_some() {
            return Err(AdapterFailure::permanent(
                "apple_configuration_unsupported",
                None,
            ));
        }
        if !valid_numeric_id(&lease.locator) {
            return Err(AdapterFailure::permanent("apple_artist_id_invalid", None));
        }
        let storefront = lease
            .storefront
            .as_deref()
            .filter(|value| valid_storefront(value))
            .ok_or_else(|| AdapterFailure::permanent("apple_storefront_invalid", None))?;
        if lease
            .locale
            .as_deref()
            .is_some_and(|value| !valid_locale(value))
        {
            return Err(AdapterFailure::permanent("apple_locale_invalid", None));
        }
        Ok(Self {
            storefront: storefront.to_owned(),
            artist_id: lease.locator.clone(),
            locale: lease.locale.clone(),
            albums_path: format!("/v1/catalog/{storefront}/artists/{}/albums", lease.locator),
        })
    }

    fn request_url(&self, cursor: Option<&str>) -> Result<Url, AdapterFailure> {
        if let Some(cursor) = cursor {
            self.validate_cursor(cursor).map(|(url, _)| url)
        } else {
            let mut url = Url::parse(&format!("{APPLE_MUSIC_ORIGIN}{}", self.albums_path))
                .map_err(|_| AdapterFailure::permanent("apple_endpoint_invalid", None))?;
            {
                let mut query = url.query_pairs_mut();
                query.append_pair("limit", &PAGE_LIMIT.to_string());
                if let Some(locale) = &self.locale {
                    query.append_pair("l", locale);
                }
            }
            Ok(url)
        }
    }

    fn validate_cursor(&self, cursor: &str) -> Result<(Url, u64), AdapterFailure> {
        if cursor.is_empty() || cursor.len() > MAX_CURSOR_BYTES {
            return Err(AdapterFailure::permanent("apple_cursor_invalid", None));
        }
        let base = Url::parse(&format!("{APPLE_MUSIC_ORIGIN}/"))
            .map_err(|_| AdapterFailure::permanent("apple_endpoint_invalid", None))?;
        let url = base
            .join(cursor)
            .map_err(|_| AdapterFailure::permanent("apple_cursor_invalid", None))?;
        validate_transport_url(&url)?;
        if url.path() != self.albums_path {
            return Err(AdapterFailure::permanent(
                "apple_cursor_scope_invalid",
                None,
            ));
        }

        let mut seen = HashSet::new();
        let mut offset = None;
        for (key, value) in url.query_pairs() {
            if !seen.insert(key.to_string()) {
                return Err(AdapterFailure::permanent(
                    "apple_cursor_query_invalid",
                    None,
                ));
            }
            match key.as_ref() {
                "offset" => {
                    offset = value.parse::<u64>().ok().filter(|value| *value > 0);
                    if offset.is_none() {
                        return Err(AdapterFailure::permanent(
                            "apple_cursor_query_invalid",
                            None,
                        ));
                    }
                }
                "limit" => {
                    let valid = value
                        .parse::<u16>()
                        .ok()
                        .is_some_and(|limit| (1..=PAGE_LIMIT).contains(&limit));
                    if !valid {
                        return Err(AdapterFailure::permanent(
                            "apple_cursor_query_invalid",
                            None,
                        ));
                    }
                }
                "l" if self.locale.as_deref() == Some(value.as_ref()) => {}
                _ => {
                    return Err(AdapterFailure::permanent(
                        "apple_cursor_query_invalid",
                        None,
                    ));
                }
            }
        }
        let offset =
            offset.ok_or_else(|| AdapterFailure::permanent("apple_cursor_query_invalid", None))?;
        Ok((url, offset))
    }

    fn album_url(&self, id: &str, href: &str) -> Result<String, AdapterFailure> {
        if !valid_numeric_id(id) {
            return Err(AdapterFailure::permanent("apple_album_id_invalid", None));
        }
        let base = Url::parse(&format!("{APPLE_MUSIC_ORIGIN}/"))
            .map_err(|_| AdapterFailure::permanent("apple_endpoint_invalid", None))?;
        let url = base
            .join(href)
            .map_err(|_| AdapterFailure::permanent("apple_album_href_invalid", None))?;
        validate_transport_url(&url)?;
        let expected = format!("/v1/catalog/{}/albums/{id}", self.storefront);
        if url.path() != expected || url.query().is_some() {
            return Err(AdapterFailure::permanent("apple_album_href_invalid", None));
        }
        Ok(url.to_string())
    }
}

fn valid_numeric_id(value: &str) -> bool {
    (1..=32).contains(&value.len())
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && value.bytes().any(|byte| byte != b'0')
}

fn valid_storefront(value: &str) -> bool {
    value.len() == 2 && value.bytes().all(|byte| byte.is_ascii_lowercase())
}

fn valid_locale(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 35
        && value.is_ascii()
        && !value.contains('_')
        && value.parse::<unic_langid::LanguageIdentifier>().is_ok()
}

fn validate_secret_ref(value: Option<&str>) -> Result<&str, AdapterFailure> {
    value
        .filter(|value| {
            !value.is_empty()
                && value.len() <= MAX_SECRET_REF_BYTES
                && !value.chars().any(char::is_control)
        })
        .ok_or_else(|| AdapterFailure::permanent("apple_secret_ref_invalid", None))
}

#[derive(Deserialize)]
struct AppleRelationshipPage<'a> {
    #[serde(borrow)]
    data: Vec<&'a RawValue>,
    #[serde(default)]
    next: Option<&'a str>,
}

#[derive(Deserialize)]
struct AppleAlbumResource<'a> {
    id: &'a str,
    #[serde(rename = "type")]
    resource_type: &'a str,
    href: &'a str,
    #[serde(borrow)]
    attributes: &'a RawValue,
}

#[derive(Deserialize)]
struct RequiredAlbumAttributes<'a> {
    name: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ParsedAppleMusicRelease<'a> {
    schema_version: u8,
    source: &'static str,
    storefront: &'a str,
    artist_id: &'a str,
    external_release_id: &'a str,
    resource_type: &'a str,
    attributes: &'a RawValue,
}

fn parse_page(
    scope: &AppleMusicScope,
    cursor: Option<&str>,
    body: &str,
) -> Result<AdapterPage, AdapterFailure> {
    let page: AppleRelationshipPage<'_> = serde_json::from_str(body)
        .map_err(|_| AdapterFailure::permanent("apple_response_schema_invalid", Some(200)))?;
    if page.data.is_empty() && page.next.is_some() {
        return Err(AdapterFailure::permanent(
            "apple_empty_page_with_next",
            Some(200),
        ));
    }

    let current_offset = cursor
        .map(|cursor| scope.validate_cursor(cursor).map(|(_, offset)| offset))
        .transpose()?
        .unwrap_or(0);
    let next_cursor = if let Some(next) = page.next {
        let (_, next_offset) = scope.validate_cursor(next)?;
        if next_offset <= current_offset {
            return Err(AdapterFailure::permanent(
                "apple_cursor_not_forward",
                Some(200),
            ));
        }
        Some(next.to_owned())
    } else {
        None
    };

    let mut ids = HashSet::new();
    let mut observations = Vec::with_capacity(page.data.len());
    for raw in page.data {
        let resource: AppleAlbumResource<'_> = serde_json::from_str(raw.get())
            .map_err(|_| AdapterFailure::permanent("apple_album_schema_invalid", Some(200)))?;
        if resource.resource_type != "albums" || !ids.insert(resource.id) {
            return Err(AdapterFailure::permanent(
                "apple_album_resource_invalid",
                Some(200),
            ));
        }
        let required: RequiredAlbumAttributes<'_> = serde_json::from_str(resource.attributes.get())
            .map_err(|_| AdapterFailure::permanent("apple_album_schema_invalid", Some(200)))?;
        if required.name.is_empty() {
            return Err(AdapterFailure::permanent(
                "apple_album_schema_invalid",
                Some(200),
            ));
        }
        let source_url = scope.album_url(resource.id, resource.href)?;
        let parsed_document = serde_json::to_string(&ParsedAppleMusicRelease {
            schema_version: 1,
            source: "appleMusic",
            storefront: &scope.storefront,
            artist_id: &scope.artist_id,
            external_release_id: resource.id,
            resource_type: resource.resource_type,
            attributes: resource.attributes,
        })
        .map_err(|_| AdapterFailure::permanent("apple_parsed_document_failed", None))?;
        observations.push(AdapterObservation {
            external_release_id: resource.id.to_owned(),
            source_url,
            raw_document: raw.get().to_owned(),
            parsed_document,
        });
    }

    let complete = next_cursor.is_none();
    Ok(AdapterPage {
        empty_full_snapshot_confirmed: cursor.is_none() && complete && observations.is_empty(),
        observations,
        next_cursor,
        checkpoint: None,
        // Every page declares the same relationship scope. The runner only
        // completes it after following `next` to the end, and Annim only
        // permits absence inference when the durable run started at root.
        coverage: SyncCoverage::FullSnapshot,
        complete,
    })
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
    };

    use annim::catalog::CatalogRowVersion;
    use chrono::Utc;
    use uuid::Uuid;

    use super::*;

    const TOKEN: &str = "eyJhbGciOiJFUzI1NiJ9.eyJpc3MiOiJURUFNSUQifQ.c2lnbmF0dXJl";
    const FIRST_RAW: &str = r#"{"id":"100","type":"albums","href":"/v1/catalog/jp/albums/100","attributes":{"name":"作品・A〜B～C（初回）","artistName":"歌手・甲"}}"#;

    struct FixtureSecrets {
        requested: Mutex<Vec<String>>,
    }

    impl FixtureSecrets {
        fn new() -> Self {
            Self {
                requested: Mutex::new(Vec::new()),
            }
        }
    }

    impl SecretResolver for FixtureSecrets {
        fn resolve<'a>(&'a self, secret_ref: &'a str) -> SecretFuture<'a> {
            self.requested.lock().unwrap().push(secret_ref.to_owned());
            Box::pin(async { Ok(SecretValue::new(TOKEN).unwrap()) })
        }
    }

    struct FixtureTransport {
        responses: Mutex<VecDeque<Result<AppleMusicHttpResponse, AdapterFailure>>>,
        requests: Mutex<Vec<AppleMusicHttpRequest>>,
    }

    impl FixtureTransport {
        fn new(
            responses: impl IntoIterator<Item = Result<AppleMusicHttpResponse, AdapterFailure>>,
        ) -> Self {
            Self {
                responses: Mutex::new(responses.into_iter().collect()),
                requests: Mutex::new(Vec::new()),
            }
        }
    }

    impl AppleMusicTransport for FixtureTransport {
        fn get<'a>(&'a self, request: AppleMusicHttpRequest) -> AppleMusicTransportFuture<'a> {
            self.requests.lock().unwrap().push(request);
            let response = self.responses.lock().unwrap().pop_front().unwrap();
            Box::pin(async move { response })
        }
    }

    fn lease() -> CatalogSyncLease {
        CatalogSyncLease {
            run_id: Uuid::new_v4(),
            source_id: Uuid::new_v4(),
            kind: CatalogSourceKind::AppleMusic,
            locator: "123".to_owned(),
            storefront: Some("jp".to_owned()),
            locale: Some("ja-JP".to_owned()),
            configuration_document: None,
            secret_ref: Some("secret/apple/developer-token".to_owned()),
            requested_cursor: None,
            lease_token: Uuid::new_v4(),
            lease_expires_at: Utc::now() + chrono::Duration::minutes(10),
            attempt_count: 1,
            row_version: CatalogRowVersion::INITIAL,
        }
    }

    #[tokio::test]
    async fn preserves_exact_release_fragments_and_follows_only_the_same_relationship() {
        let next = "/v1/catalog/jp/artists/123/albums?offset=100&limit=100&l=ja-JP";
        let first_body = format!(r#"{{"data":[{FIRST_RAW}],"next":"{next}"}}"#);
        let second_raw = r#"{"id":"200","type":"albums","href":"/v1/catalog/jp/albums/200","attributes":{"name":"第二作（通常盤）","artistName":"歌手・甲"}}"#;
        let second_body = format!(r#"{{"data":[{second_raw}]}}"#);
        let secrets = Arc::new(FixtureSecrets::new());
        let transport = FixtureTransport::new([
            Ok(AppleMusicHttpResponse::json(200, first_body, None)),
            Ok(AppleMusicHttpResponse::json(200, second_body, None)),
        ]);
        let adapter = AppleMusicAdapter::with_transport(secrets.clone(), transport);
        let lease = lease();

        let first = adapter.fetch_page_inner(&lease, None).await.unwrap();
        assert_eq!(first.coverage, SyncCoverage::FullSnapshot);
        assert!(!first.complete);
        assert_eq!(first.next_cursor.as_deref(), Some(next));
        assert_eq!(first.observations[0].raw_document, FIRST_RAW);
        assert!(first.observations[0]
            .parsed_document
            .contains("作品・A〜B～C（初回）"));
        assert!(first.observations[0]
            .parsed_document
            .contains("\"schemaVersion\":1"));

        let second = adapter.fetch_page_inner(&lease, Some(next)).await.unwrap();
        assert!(second.complete);
        assert_eq!(second.observations[0].raw_document, second_raw);

        let requests = adapter.transport.requests.lock().unwrap();
        assert_eq!(
            requests[0].url.as_str(),
            "https://api.music.apple.com/v1/catalog/jp/artists/123/albums?limit=100&l=ja-JP"
        );
        assert_eq!(
            requests[1].url.as_str(),
            format!("{APPLE_MUSIC_ORIGIN}{next}")
        );
        assert_eq!(requests[0].developer_token.expose_secret(), TOKEN);
        assert!(!format!("{:?}", requests[0]).contains(TOKEN));
        assert_eq!(secrets.requested.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn rejects_invalid_scope_before_resolving_a_secret_or_using_transport() {
        let secrets = Arc::new(FixtureSecrets::new());
        let transport = FixtureTransport::new([]);
        let adapter = AppleMusicAdapter::with_transport(secrets.clone(), transport);
        let mut invalid_lease = lease();
        invalid_lease.locale = Some("ja_JP".to_owned());
        let failure = adapter
            .fetch_page_inner(&invalid_lease, None)
            .await
            .unwrap_err();
        assert_eq!(failure.code(), "apple_locale_invalid");

        let mut invalid_lease = lease();
        invalid_lease.storefront = Some("JP".to_owned());
        let failure = adapter
            .fetch_page_inner(&invalid_lease, None)
            .await
            .unwrap_err();
        assert_eq!(failure.code(), "apple_storefront_invalid");

        let mut invalid_lease = lease();
        invalid_lease.locator = "../123".to_owned();
        let failure = adapter
            .fetch_page_inner(&invalid_lease, None)
            .await
            .unwrap_err();
        assert_eq!(failure.code(), "apple_artist_id_invalid");

        let lease = lease();
        for cursor in [
            "https://example.com/v1/catalog/jp/artists/123/albums?offset=100",
            "/v1/catalog/jp/artists/999/albums?offset=100",
            "/v1/catalog/jp/artists/123/albums?offset=100&include=tracks",
        ] {
            let failure = adapter
                .fetch_page_inner(&lease, Some(cursor))
                .await
                .unwrap_err();
            assert!(matches!(
                failure.code(),
                "apple_request_origin_invalid"
                    | "apple_cursor_scope_invalid"
                    | "apple_cursor_query_invalid"
            ));
        }
        assert!(secrets.requested.lock().unwrap().is_empty());
        assert!(adapter.transport.requests.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn maps_rate_limits_and_confirms_only_a_proven_empty_root_snapshot() {
        let secrets = Arc::new(FixtureSecrets::new());
        let transport = FixtureTransport::new([
            Ok(AppleMusicHttpResponse::json(
                429,
                r#"{"errors":[]}"#,
                Some(Duration::from_secs(120)),
            )),
            Ok(AppleMusicHttpResponse::json(200, r#"{"data":[]}"#, None)),
        ]);
        let adapter = AppleMusicAdapter::with_transport(secrets, transport);
        let lease = lease();

        let failure = adapter.fetch_page_inner(&lease, None).await.unwrap_err();
        assert_eq!(failure.code(), "apple_rate_limited");
        assert_eq!(failure.http_status(), Some(429));
        assert_eq!(failure.retry_after(), Some(Duration::from_secs(120)));

        let page = adapter.fetch_page_inner(&lease, None).await.unwrap();
        assert!(page.complete);
        assert!(page.empty_full_snapshot_confirmed);
    }

    #[tokio::test]
    async fn production_transport_rejects_off_origin_without_network_access() {
        let transport = AppleMusicHttpTransport::new(AppleMusicHttpPolicy::default()).unwrap();
        let request = AppleMusicHttpRequest {
            url: Url::parse("https://example.com/v1/catalog/jp/artists/123/albums").unwrap(),
            developer_token: SecretValue::new(TOKEN).unwrap(),
        };
        let failure = transport.get(request).await.unwrap_err();
        assert_eq!(failure.code(), "apple_request_origin_invalid");

        assert_eq!(
            parse_retry_after("120", SystemTime::UNIX_EPOCH),
            Some(Duration::from_secs(120))
        );
        let date = "Thu, 01 Jan 1970 00:02:00 GMT";
        assert_eq!(
            parse_retry_after(date, SystemTime::UNIX_EPOCH),
            Some(Duration::from_secs(120))
        );
        assert!(SecretValue::new("secret-token").is_err());
        assert!(!format!("{:?}", SecretValue::new(TOKEN).unwrap()).contains(TOKEN));
    }
}
