use std::sync::LazyLock;

/// https://github.com/async-graphql/examples/blob/0c4e5e29e97a41c8877c126cbcefb82721ae81af/models/token/src/lib.rs
use async_graphql::{Data, Result};
use serde::Deserialize;

static TOKEN: LazyLock<String> =
    LazyLock::new(|| std::env::var("ANNIM_AUTH_TOKEN").unwrap_or_else(|_| "114514".to_string()));

pub struct AuthToken(String);

impl AuthToken {
    pub fn new(token: String) -> Self {
        Self(token)
    }

    pub fn is_valid(&self) -> bool {
        self.0 == TOKEN.as_str()
    }
}

pub async fn on_connection_init(value: serde_json::Value) -> Result<Data> {
    #[derive(Deserialize)]
    struct Payload {
        token: String,
    }

    // Coerce the connection params into our `Payload` struct so we can
    // validate the token exists in the headers.
    if let Ok(payload) = serde_json::from_value::<Payload>(value) {
        let mut data = Data::default();
        data.insert(AuthToken(payload.token));
        Ok(data)
    } else {
        Err("Token is required".into())
    }
}

pub(crate) struct AdminGuard;

impl async_graphql::Guard for AdminGuard {
    async fn check(&self, ctx: &async_graphql::Context<'_>) -> async_graphql::Result<()> {
        let token = ctx
            .data::<AuthToken>()
            .map_err(|_| async_graphql::Error::new("Token is required"))?;
        if !token.is_valid() {
            return Err(async_graphql::Error::new("Invalid token"));
        }

        Ok(())
    }
}
