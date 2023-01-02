use jwt_simple::prelude::HS256Key;
use tokio::sync::RwLock;

/// Readonly keys
pub struct AnnilKeys {
    pub sign_key: HS256Key,
    pub share_key: HS256Key,
    pub admin_token: String,
}

impl AnnilKeys {
    pub fn new(sign_key: &[u8], share_key: &[u8], admin_token: String) -> Self {
        Self {
            sign_key: HS256Key::from_bytes(sign_key),
            share_key: HS256Key::from_bytes(share_key),
            admin_token,
        }
    }
}

pub struct AnnilState {
    pub version: String,
    pub last_update: RwLock<u64>,
    pub etag: RwLock<String>,

    #[cfg(feature = "metadata")]
    pub metadata: Option<crate::metadata::MetadataConfig>,
}
