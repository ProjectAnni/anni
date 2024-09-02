use std::sync::LazyLock;

/// https://github.com/async-graphql/examples/blob/0c4e5e29e97a41c8877c126cbcefb82721ae81af/models/token/src/lib.rs
use async_graphql::{Context, Data, Result};
use serde::Deserialize;

static TOKEN: LazyLock<String> =
    LazyLock::new(|| std::env::var("ANNIM_AUTH_TOKEN").unwrap_or_else(|_| "114514".to_string()));

pub struct AuthToken(pub String);

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

pub fn require_auth<'ctx>(ctx: &Context<'ctx>) -> anyhow::Result<()> {
    let token = ctx
        .data::<AuthToken>()
        .map_err(|_| anyhow::anyhow!("Token is required"))?;
    if token.0 != TOKEN.as_str() {
        anyhow::bail!("Invalid token");
    }

    Ok(())
}
