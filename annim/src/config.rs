use std::{
    collections::HashSet,
    fmt,
    net::{AddrParseError, SocketAddr},
    path::{Component, Path},
    sync::Arc,
};

use axum::http::HeaderValue;
use thiserror::Error;
use url::Url;

pub const DEFAULT_BIND_ADDR: &str = "127.0.0.1:8000";
pub const APPLE_MUSIC_SECRET_REF_ENV: &str = "ANNIM_CATALOG_APPLE_MUSIC_SECRET_REF";

const MAX_CATALOG_SECRET_REF_BYTES: usize = 512;

/// Trusted server-side bindings used when the browser registers catalog sources.
///
/// Values are opaque references into the catalog worker's secret root. The
/// browser never submits or receives these references, and Annim never reads
/// the referenced secret file.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct CatalogSyncProvisioningConfig {
    apple_music_secret_ref: Option<CatalogSecretRef>,
}

impl fmt::Debug for CatalogSyncProvisioningConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CatalogSyncProvisioningConfig")
            .field(
                "apple_music_secret_ref",
                &self.apple_music_secret_ref.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

impl CatalogSyncProvisioningConfig {
    pub fn from_env() -> Result<Self, CatalogSyncProvisioningConfigError> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    pub fn from_lookup(
        mut lookup: impl FnMut(&str) -> Option<String>,
    ) -> Result<Self, CatalogSyncProvisioningConfigError> {
        let apple_music_secret_ref = lookup(APPLE_MUSIC_SECRET_REF_ENV)
            .map(|value| CatalogSecretRef::parse(APPLE_MUSIC_SECRET_REF_ENV, value))
            .transpose()?;
        Ok(Self {
            apple_music_secret_ref,
        })
    }

    pub fn apple_music_secret_ref(&self) -> Option<&CatalogSecretRef> {
        self.apple_music_secret_ref.as_ref()
    }
}

/// An opaque relative reference into the catalog worker's secret root.
///
/// It intentionally implements neither `Display` nor serialization so a
/// reference cannot be logged or placed in an API response by convenience.
#[derive(Clone, PartialEq, Eq)]
pub struct CatalogSecretRef(String);

impl fmt::Debug for CatalogSecretRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}

impl CatalogSecretRef {
    pub fn parse(
        name: &'static str,
        value: String,
    ) -> Result<Self, CatalogSyncProvisioningConfigError> {
        validate_catalog_secret_reference(name, &value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Error)]
pub enum CatalogSyncProvisioningConfigError {
    #[error(
        "{name} must be a relative secret file reference without traversal, control characters, or backslashes"
    )]
    InvalidSecretReference { name: &'static str },
}

fn validate_catalog_secret_reference(
    name: &'static str,
    value: &str,
) -> Result<(), CatalogSyncProvisioningConfigError> {
    let path = Path::new(value);
    let valid = !value.is_empty()
        && value.len() <= MAX_CATALOG_SECRET_REF_BYTES
        && !value.contains('\\')
        && !value.chars().any(char::is_control)
        && !looks_like_compact_jwt(value)
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)));
    if valid {
        Ok(())
    } else {
        Err(CatalogSyncProvisioningConfigError::InvalidSecretReference { name })
    }
}

fn looks_like_compact_jwt(value: &str) -> bool {
    if value.contains('/') {
        return false;
    }
    let segments = value.split('.').collect::<Vec<_>>();
    segments.len() == 3
        && segments.iter().all(|segment| {
            !segment.is_empty()
                && segment.chars().all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
                })
        })
}

/// Network-facing settings for Annim.
///
/// The default listener is loopback-only. Deployments that bind a non-loopback
/// address should terminate TLS at a trusted reverse proxy and explicitly set
/// every browser origin that may access the API.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerConfig {
    bind_addr: SocketAddr,
    allowed_origins: Arc<[HeaderValue]>,
    graphiql_enabled: bool,
}

impl ServerConfig {
    pub fn from_env() -> Result<Self, ServerConfigError> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    pub fn from_lookup(
        mut lookup: impl FnMut(&str) -> Option<String>,
    ) -> Result<Self, ServerConfigError> {
        let bind_addr = lookup("ANNIM_BIND_ADDR")
            .unwrap_or_else(|| DEFAULT_BIND_ADDR.to_owned())
            .parse()
            .map_err(ServerConfigError::InvalidBindAddress)?;
        let allowed_origins = parse_allowed_origins(
            lookup("ANNIM_ALLOWED_ORIGINS")
                .as_deref()
                .unwrap_or_default(),
        )?;
        let graphiql_enabled = parse_boolean(
            "ANNIM_GRAPHIQL_ENABLED",
            lookup("ANNIM_GRAPHIQL_ENABLED").as_deref(),
            false,
        )?;

        Ok(Self {
            bind_addr,
            allowed_origins: allowed_origins.into(),
            graphiql_enabled,
        })
    }

    pub fn bind_addr(&self) -> SocketAddr {
        self.bind_addr
    }

    pub fn allowed_origins(&self) -> &[HeaderValue] {
        &self.allowed_origins
    }

    pub fn graphiql_enabled(&self) -> bool {
        self.graphiql_enabled
    }
}

#[derive(Debug, Error)]
pub enum ServerConfigError {
    #[error("ANNIM_BIND_ADDR must be a socket address with an explicit port")]
    InvalidBindAddress(#[source] AddrParseError),
    #[error("ANNIM_ALLOWED_ORIGINS contains an invalid origin")]
    InvalidOrigin,
    #[error("{name} must be either true or false, got {value}")]
    InvalidBoolean { name: &'static str, value: String },
}

fn parse_allowed_origins(value: &str) -> Result<Vec<HeaderValue>, ServerConfigError> {
    let mut unique = HashSet::new();
    let mut origins = Vec::new();

    for candidate in value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        let parsed = Url::parse(candidate).map_err(|_| ServerConfigError::InvalidOrigin)?;
        let canonical = parsed.origin().ascii_serialization();
        let valid = matches!(parsed.scheme(), "http" | "https")
            && parsed.username().is_empty()
            && parsed.password().is_none()
            && parsed.host().is_some()
            && parsed.path() == "/"
            && parsed.query().is_none()
            && parsed.fragment().is_none()
            && canonical == candidate;
        if !valid {
            return Err(ServerConfigError::InvalidOrigin);
        }
        if unique.insert(canonical.clone()) {
            origins.push(
                HeaderValue::from_str(&canonical).map_err(|_| ServerConfigError::InvalidOrigin)?,
            );
        }
    }

    Ok(origins)
}

fn parse_boolean(
    name: &'static str,
    value: Option<&str>,
    default: bool,
) -> Result<bool, ServerConfigError> {
    match value {
        None => Ok(default),
        Some("true") => Ok(true),
        Some("false") => Ok(false),
        Some(value) => Err(ServerConfigError::InvalidBoolean {
            name,
            value: value.to_owned(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_loopback_only_and_disable_browser_features() {
        let config = ServerConfig::from_lookup(|_| None).unwrap();

        assert_eq!(config.bind_addr().to_string(), DEFAULT_BIND_ADDR);
        assert!(config.allowed_origins().is_empty());
        assert!(!config.graphiql_enabled());
    }

    #[test]
    fn explicit_origins_are_canonical_exact_and_deduplicated() {
        let config = ServerConfig::from_lookup(|name| match name {
            "ANNIM_BIND_ADDR" => Some("[::1]:9000".to_owned()),
            "ANNIM_ALLOWED_ORIGINS" => {
                Some("https://ui.example,http://127.0.0.1:5173,https://ui.example".to_owned())
            }
            "ANNIM_GRAPHIQL_ENABLED" => Some("true".to_owned()),
            _ => None,
        })
        .unwrap();

        assert_eq!(config.bind_addr().to_string(), "[::1]:9000");
        assert_eq!(
            config.allowed_origins(),
            ["https://ui.example", "http://127.0.0.1:5173"]
        );
        assert!(config.graphiql_enabled());
    }

    #[test]
    fn invalid_network_configuration_fails_closed() {
        for origin in [
            "*",
            "null",
            "https://ui.example/",
            "https://ui.example/path",
            "https://user@ui.example",
        ] {
            assert!(matches!(
                ServerConfig::from_lookup(
                    |name| (name == "ANNIM_ALLOWED_ORIGINS").then(|| origin.to_owned())
                ),
                Err(ServerConfigError::InvalidOrigin)
            ));
        }
        assert!(matches!(
            ServerConfig::from_lookup(
                |name| (name == "ANNIM_BIND_ADDR").then(|| "0.0.0.0".to_owned())
            ),
            Err(ServerConfigError::InvalidBindAddress(_))
        ));
        assert!(matches!(
            ServerConfig::from_lookup(
                |name| (name == "ANNIM_GRAPHIQL_ENABLED").then(|| "yes".to_owned())
            ),
            Err(ServerConfigError::InvalidBoolean { .. })
        ));

        let secret_origin = "https://operator:do-not-log@ui.example";
        let error = ServerConfig::from_lookup(|name| {
            (name == "ANNIM_ALLOWED_ORIGINS").then(|| secret_origin.to_owned())
        })
        .unwrap_err();
        let rendered = format!("{error:?}: {error}");
        assert!(!rendered.contains("do-not-log"));
    }

    #[test]
    fn catalog_provisioning_accepts_only_redacted_relative_secret_references() {
        let secret_ref = "apple/developer-token.jwt";
        let configured = CatalogSyncProvisioningConfig::from_lookup(|name| {
            (name == APPLE_MUSIC_SECRET_REF_ENV).then(|| secret_ref.to_owned())
        })
        .unwrap();
        assert_eq!(
            configured
                .apple_music_secret_ref()
                .map(CatalogSecretRef::as_str),
            Some(secret_ref)
        );
        assert!(!format!("{:?}", configured).contains(secret_ref));

        let missing = CatalogSyncProvisioningConfig::from_lookup(|_| None).unwrap();
        assert!(missing.apple_music_secret_ref().is_none());

        for invalid in [
            "",
            "/run/secrets/apple.jwt",
            "../apple.jwt",
            "apple/../developer-token.jwt",
            ".",
            "apple\\developer-token.jwt",
            "apple/developer-token.jwt\n",
            "eyJhbGciOiJFUzI1NiJ9.eyJpc3MiOiJpc3N1ZXIifQ.signature",
        ] {
            assert!(matches!(
                CatalogSyncProvisioningConfig::from_lookup(|name| {
                    (name == APPLE_MUSIC_SECRET_REF_ENV).then(|| invalid.to_owned())
                }),
                Err(CatalogSyncProvisioningConfigError::InvalidSecretReference { .. })
            ));
        }

        let too_long = "a".repeat(MAX_CATALOG_SECRET_REF_BYTES + 1);
        assert!(matches!(
            CatalogSyncProvisioningConfig::from_lookup(|name| {
                (name == APPLE_MUSIC_SECRET_REF_ENV).then(|| too_long.clone())
            }),
            Err(CatalogSyncProvisioningConfigError::InvalidSecretReference { .. })
        ));
    }
}
