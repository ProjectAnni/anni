pub mod auth;
pub mod config;
pub mod error;
pub mod provider;
pub mod services;
pub mod utils;

use crate::config::MetadataConfig;
use crate::provider::AnnilProvider;
pub use actix_cors;
pub use actix_utils;
pub use actix_web;
use jwt_simple::prelude::HS256Key;
use parking_lot::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct AppState {
    pub providers: RwLock<Vec<AnnilProvider>>,
    pub key: HS256Key,
    pub share_key: HS256Key,
    pub admin_token: String,

    pub version: String,
    pub metadata: Option<MetadataConfig>,
    pub last_update: RwLock<u64>,
    pub etag: RwLock<Option<String>>,
}

impl Default for AppState {
    fn default() -> Self {
        let last_update = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            providers: RwLock::new(Vec::new()),
            key: HS256Key::generate(),
            share_key: HS256Key::generate(),
            admin_token: "".to_string(),
            version: format!("Annil v{}", env!("CARGO_PKG_VERSION")),
            metadata: None,
            last_update: RwLock::new(last_update),
            etag: RwLock::new(None),
        }
    }
}
