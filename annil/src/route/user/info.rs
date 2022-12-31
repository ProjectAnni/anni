use crate::state::AnnilState;
use axum::{Extension, Json};
use jwt_simple::reexports::serde_json::{json, Value};
use std::sync::Arc;

pub async fn info(Extension(data): Extension<Arc<AnnilState>>) -> Json<Value> {
    Json(json!({
        "version": data.version,
        "protocol_version": "0.4.1",
        "last_update": *data.last_update.read().await,
    }))
}
