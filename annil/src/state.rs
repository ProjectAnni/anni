use crate::config::MetadataConfig;
use crate::provider::AnnilProvider;
use jwt_simple::prelude::HS256Key;
use std::ops::Deref;
use tokio::sync::RwLock;

/// Readonly keys
pub struct AnnilKeys {
    pub sign_key: HS256Key,
    pub share_key: HS256Key,
    pub admin_token: String,
}

pub struct AnnilProviders(pub RwLock<Vec<AnnilProvider>>);

impl Deref for AnnilProviders {
    type Target = RwLock<Vec<AnnilProvider>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct AnnilState {
    pub version: String,
    pub last_update: RwLock<u64>,
    pub etag: RwLock<String>,

    pub metadata: Option<MetadataConfig>,
}
