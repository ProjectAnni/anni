use config::{Config, ProviderItem};

use anni_provider::cache::{Cache, CachePool};
use anni_provider::fs::LocalFileSystemProvider;
use anni_provider::providers::drive::DriveProviderSettings;
use anni_provider::providers::{CommonConventionProvider, CommonStrictProvider, DriveProvider};
use anni_provider::AnniProvider;
use annil::metadata::MetadataConfig;
use annil::provider::AnnilProvider;
use annil::route::admin;
use annil::route::user;
use annil::state::{AnnilKeys, AnnilProviders, AnnilState};
use axum::http::Method;
use axum::routing::{get, post};
use axum::{Extension, Router, Server};
use jwt_simple::prelude::HS256Key;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tower_http::cors;
use tower_http::cors::CorsLayer;

async fn init_state(config: Config) -> anyhow::Result<(AnnilState, AnnilProviders, AnnilKeys)> {
    #[cfg(feature = "metadata")]
    let mut db = config.metadata.clone().map(MetadataConfig::into_db);

    log::info!("Start initializing providers...");
    let now = SystemTime::now();
    let mut providers = Vec::with_capacity(config.providers.len());
    let mut caches = HashMap::new();

    for (provider_name, provider_config) in config.providers.iter() {
        log::debug!("Initializing provider: {}", provider_name);
        let mut provider: Box<dyn AnniProvider + Send + Sync> =
            match (&provider_config.item, &mut db) {
                (
                    ProviderItem::File {
                        root,
                        strict: false,
                        ..
                    },
                    Some(db),
                ) => Box::new(
                    CommonConventionProvider::new(
                        PathBuf::from(root),
                        db.open()?,
                        Box::new(LocalFileSystemProvider),
                    )
                    .await?,
                ),
                (
                    ProviderItem::File {
                        root,
                        strict: true,
                        layer,
                    },
                    _,
                ) => Box::new(
                    CommonStrictProvider::new(
                        PathBuf::from(root),
                        *layer,
                        Box::new(LocalFileSystemProvider),
                    )
                    .await?,
                ),
                (
                    ProviderItem::Drive {
                        drive_id,
                        corpora,
                        initial_token_path,
                        token_path,
                        strict: false,
                    },
                    Some(db),
                ) => {
                    if let Some(initial_token_path) = initial_token_path {
                        if initial_token_path.exists() && !token_path.exists() {
                            let _ = std::fs::copy(initial_token_path, token_path.clone());
                        }
                    }
                    Box::new(
                        DriveProvider::new(
                            Default::default(),
                            DriveProviderSettings {
                                corpora: corpora.to_string(),
                                drive_id: drive_id.clone(),
                            },
                            Some(db.open()?),
                            token_path.clone(),
                        )
                        .await?,
                    )
                }
                (
                    ProviderItem::Drive {
                        drive_id,
                        corpora,
                        initial_token_path,
                        token_path,
                        strict: true,
                    },
                    _,
                ) => {
                    if let Some(initial_token_path) = initial_token_path {
                        if initial_token_path.exists() && !token_path.exists() {
                            let _ = std::fs::copy(initial_token_path, token_path.clone());
                        }
                    }
                    Box::new(
                        DriveProvider::new(
                            Default::default(),
                            DriveProviderSettings {
                                corpora: corpora.to_string(),
                                drive_id: drive_id.clone(),
                            },
                            None,
                            token_path.clone(),
                        )
                        .await?,
                    )
                }
                (_, None) => {
                    log::error!(
                        "Metadata is not configured, but provider {} requires it.",
                        provider_name
                    );
                    continue;
                }
            };
        if let Some(cache) = provider_config.cache() {
            log::debug!(
                "Cache configuration detected: root = {}, max-size = {}",
                cache.root,
                cache.max_size
            );
            if !caches.contains_key(&cache.root) {
                // new cache pool
                let pool = CachePool::new(&cache.root, cache.max_size);
                caches.insert(cache.root.to_string(), Arc::new(pool));
            }
            provider = Box::new(Cache::new(provider, caches[&cache.root].clone()));
        }
        let provider = AnnilProvider::new(provider_name.to_string(), provider);
        providers.push(provider);
    }
    log::info!(
        "Provider initialization finished, used {:?}",
        now.elapsed().unwrap()
    );

    let providers = AnnilProviders(RwLock::new(providers));
    let etag = providers.compute_etag().await;

    // key
    let sign_key = HS256Key::from_bytes(config.server.sign_key.as_ref());
    let share_key = HS256Key::from_bytes(config.server.share_key.as_ref())
        .with_key_id(&config.server.share_key_id);
    let version = format!("Annil v{}", env!("CARGO_PKG_VERSION"));
    let last_update = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    Ok((
        AnnilState {
            version,
            metadata: config.metadata,
            last_update: RwLock::new(last_update),
            etag: RwLock::new(etag),
        },
        providers,
        AnnilKeys {
            sign_key,
            share_key,
            admin_token: config.server.admin_token,
        },
    ))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .parse_env("ANNI_LOG")
        .filter_module("sqlx::query", log::LevelFilter::Warn)
        .init();
    let config = Config::from_file(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "config.toml".to_owned()),
    )?;
    let listen: SocketAddr = config.server.listen.parse()?;
    let (state, providers, keys) = init_state(config).await?;

    let app = Router::new()
        .route("/info", get(user::info))
        .route("/albums", get(user::albums))
        .route(
            "/:album_id/:disc_id/:track_id",
            get(user::audio).head(user::audio_head),
        )
        .route("/cover/:album_id", get(user::cover))
        .route("/cover/:album_id/:disc_id", get(user::cover))
        .layer(
            CorsLayer::new()
                .allow_methods([Method::GET])
                .allow_origin(cors::Any)
                .allow_headers(cors::Any),
        )
        .route("/admin/sign", post(admin::sign))
        .route("/admin/reload", post(admin::reload))
        .layer(Extension(Arc::new(state)))
        .layer(Extension(Arc::new(providers)))
        .layer(Extension(Arc::new(keys)));

    Server::bind(&listen)
        .serve(app.into_make_service())
        .await
        .unwrap();

    Ok(())
}

mod config {
    use annil::metadata::MetadataConfig;
    use serde::Deserialize;
    use std::collections::HashMap;
    use std::fs;
    use std::path::{Path, PathBuf};

    #[derive(Deserialize)]
    pub struct Config {
        pub server: ServerConfig,
        pub metadata: Option<MetadataConfig>,
        #[serde(rename = "backends")]
        pub providers: HashMap<String, ProviderConfig>,
    }

    impl Config {
        pub fn from_file<P: AsRef<Path>>(config_path: P) -> anyhow::Result<Self> {
            let string = fs::read_to_string(config_path)?;
            let result = toml_edit::easy::from_str(&string)?;
            Ok(result)
        }
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "kebab-case")]
    pub struct ServerConfig {
        /// Server name
        pub name: String,
        /// Port to listen on
        pub listen: String,
        /// HMAC key for JWT
        #[serde(rename = "hmac-key")]
        pub sign_key: String,
        pub share_key: String,
        pub share_key_id: String,
        /// Password to reload data
        pub admin_token: String,
    }

    #[derive(Deserialize)]
    pub struct ProviderConfig {
        #[serde(flatten)]
        pub item: ProviderItem,
        cache: Option<CacheConfig>,
    }

    impl ProviderConfig {
        #[inline]
        pub fn cache(&self) -> Option<&CacheConfig> {
            self.cache.as_ref()
        }
    }

    #[derive(Deserialize)]
    #[serde(tag = "type")]
    pub enum ProviderItem {
        #[serde(rename = "file")]
        #[serde(rename_all = "kebab-case")]
        File {
            root: String,
            strict: bool,
            #[serde(default = "default_layer")]
            layer: usize,
        },
        #[serde(rename = "drive")]
        #[serde(rename_all = "kebab-case")]
        Drive {
            corpora: String,
            drive_id: Option<String>,
            initial_token_path: Option<PathBuf>,
            token_path: PathBuf,
            #[serde(default)]
            strict: bool,
        },
    }

    const fn default_layer() -> usize {
        2
    }

    #[derive(Deserialize)]
    pub struct CacheConfig {
        pub root: String,
        #[serde(default, rename = "max-size")]
        pub max_size: usize,
    }
}
