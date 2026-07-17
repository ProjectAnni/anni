use std::{fmt, sync::Arc};

use async_graphql::{Data, Result};
use constant_time_eq::constant_time_eq_32;
use serde::Deserialize;
use sha2::{Digest as _, Sha256};
use thiserror::Error;

const MINIMUM_TOKEN_LENGTH: usize = 32;

#[derive(Clone)]
pub struct AuthConfig {
    expected_digest: Arc<[u8; 32]>,
}

impl AuthConfig {
    pub fn new(token: impl AsRef<str>) -> std::result::Result<Self, AuthConfigError> {
        let token = token.as_ref();
        validate_configured_token(token)?;
        Ok(Self {
            expected_digest: Arc::new(Sha256::digest(token.as_bytes()).into()),
        })
    }

    pub fn from_env() -> std::result::Result<Self, AuthConfigError> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    pub fn from_lookup(
        lookup: impl FnOnce(&str) -> Option<String>,
    ) -> std::result::Result<Self, AuthConfigError> {
        let token = lookup("ANNIM_AUTH_TOKEN").ok_or(AuthConfigError::MissingToken)?;
        Self::new(token)
    }

    pub fn authenticate_bearer(
        &self,
        authorization: &str,
    ) -> std::result::Result<AuthenticatedAdmin, AuthenticationError> {
        let (scheme, token) = authorization
            .split_once(' ')
            .ok_or(AuthenticationError::Unauthorized)?;
        if !scheme.eq_ignore_ascii_case("Bearer")
            || token.is_empty()
            || token.chars().any(char::is_whitespace)
        {
            return Err(AuthenticationError::Unauthorized);
        }
        let actual: [u8; 32] = Sha256::digest(token.as_bytes()).into();
        if constant_time_eq_32(self.expected_digest.as_ref(), &actual) {
            Ok(AuthenticatedAdmin(()))
        } else {
            Err(AuthenticationError::Unauthorized)
        }
    }
}

impl fmt::Debug for AuthConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthConfig")
            .field("expected_digest", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthenticatedAdmin(());

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("Unauthorized")]
pub enum AuthenticationError {
    Unauthorized,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AuthConfigError {
    #[error("ANNIM_AUTH_TOKEN is required")]
    MissingToken,
    #[error("ANNIM_AUTH_TOKEN must contain at least {MINIMUM_TOKEN_LENGTH} characters")]
    TokenTooShort,
    #[error("ANNIM_AUTH_TOKEN cannot contain whitespace")]
    TokenContainsWhitespace,
}

fn validate_configured_token(token: &str) -> std::result::Result<(), AuthConfigError> {
    if token.chars().count() < MINIMUM_TOKEN_LENGTH {
        return Err(AuthConfigError::TokenTooShort);
    }
    if token.chars().any(char::is_whitespace) {
        return Err(AuthConfigError::TokenContainsWhitespace);
    }
    Ok(())
}

pub async fn on_connection_init(auth: AuthConfig, value: serde_json::Value) -> Result<Data> {
    #[derive(Deserialize)]
    struct Payload {
        authorization: String,
    }

    let payload = serde_json::from_value::<Payload>(value).map_err(|_| "Unauthorized")?;
    let authenticated = auth
        .authenticate_bearer(&payload.authorization)
        .map_err(|_| "Unauthorized")?;
    let mut data = Data::default();
    data.insert(authenticated);
    Ok(data)
}

pub(crate) struct AdminGuard;

impl async_graphql::Guard for AdminGuard {
    async fn check(&self, ctx: &async_graphql::Context<'_>) -> async_graphql::Result<()> {
        ctx.data::<AuthenticatedAdmin>()
            .map(|_| ())
            .map_err(|_| async_graphql::Error::new("Unauthorized"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn auth_config_requires_an_explicit_strong_token() {
        assert!(matches!(
            AuthConfig::from_lookup(|_| None),
            Err(AuthConfigError::MissingToken)
        ));
        assert!(matches!(
            AuthConfig::new("too-short"),
            Err(AuthConfigError::TokenTooShort)
        ));
        assert!(matches!(
            AuthConfig::new("0123456789abcdef 0123456789abcdef"),
            Err(AuthConfigError::TokenContainsWhitespace)
        ));
        AuthConfig::new(TOKEN).unwrap();
    }

    #[test]
    fn bearer_authentication_is_strict_and_debug_is_redacted() {
        let auth = AuthConfig::new(TOKEN).unwrap();
        assert!(auth.authenticate_bearer(&format!("Bearer {TOKEN}")).is_ok());
        assert!(auth.authenticate_bearer(&format!("bearer {TOKEN}")).is_ok());
        assert!(auth.authenticate_bearer(TOKEN).is_err());
        assert!(auth.authenticate_bearer("Bearer wrong-token").is_err());
        let debug = format!("{auth:?}");
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains(TOKEN));
    }

    #[tokio::test]
    async fn websocket_init_uses_the_same_bearer_contract() {
        let auth = AuthConfig::new(TOKEN).unwrap();
        let accepted = on_connection_init(
            auth.clone(),
            serde_json::json!({ "authorization": format!("Bearer {TOKEN}") }),
        )
        .await;
        assert!(accepted.is_ok());
        assert!(
            on_connection_init(auth, serde_json::json!({ "token": TOKEN }))
                .await
                .is_err()
        );
    }
}
