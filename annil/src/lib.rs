extern crate core;

pub mod config;
pub mod error;
pub mod extractor;
pub mod provider;
pub mod route;
pub mod utils;

use crate::config::MetadataConfig;
use crate::provider::AnnilProvider;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

pub struct AppState {
    pub providers: RwLock<Vec<AnnilProvider>>,
    pub etag: RwLock<Option<String>>,

    pub metadata: Option<MetadataConfig>,

    pub version: String,
    pub last_update: RwLock<u64>,
}

impl Default for AppState {
    fn default() -> Self {
        let last_update = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            providers: RwLock::new(Vec::new()),
            version: format!("Annil v{}", env!("CARGO_PKG_VERSION")),
            metadata: None,
            last_update: RwLock::new(last_update),
            etag: RwLock::new(None),
        }
    }
}
