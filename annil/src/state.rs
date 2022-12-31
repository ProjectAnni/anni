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

impl AnnilProviders {
    pub async fn compute_etag(&self) -> String {
        let providers = self.0.read().await;

        let mut etag = 0;
        for provider in providers.iter() {
            for album in provider.albums().await {
                if let Ok(uuid) = uuid::Uuid::parse_str(album.as_ref()) {
                    etag ^= uuid.as_u128();
                } else {
                    log::error!("Failed to parse uuid: {album}");
                }
            }
        }
        format!(r#""{}""#, base64::encode(etag.to_be_bytes()))
    }
}

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

    #[cfg(feature = "metadata")]
    pub metadata: Option<crate::metadata::MetadataConfig>,
}
