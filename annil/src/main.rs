mod auth;
mod config;
mod error;
mod provider;
mod services;
mod utils;

use crate::auth::{AnnilAuth, AnnilClaims};
use crate::config::{Config, MetadataConfig, ProviderItem};
use crate::error::AnnilError;
use crate::provider::AnnilProvider;
use crate::services::*;
use crate::utils::compute_etag;
use actix_cors::Cors;
use actix_web::middleware::Logger;
use actix_web::{web, App, HttpServer};
use anni_provider::cache::{Cache, CachePool};
use anni_provider::fs::LocalFileSystemProvider;
use anni_provider::providers::drive::DriveProviderSettings;
use anni_provider::providers::{CommonConventionProvider, CommonStrictProvider, DriveProvider};
use anni_provider::{AnniProvider, RepoDatabaseRead};
use anni_repo::{setup_git2, RepositoryManager};
use jwt_simple::prelude::HS256Key;
use jwt_simple::reexports::serde_json::json;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct AppState {
    providers: RwLock<Vec<AnnilProvider>>,
    key: HS256Key,
    share_key: HS256Key,
    admin_token: String,

    version: String,
    metadata: MetadataConfig,
    last_update: RwLock<u64>,
    etag: RwLock<String>,
}

fn init_metadata(config: &Config) -> anyhow::Result<PathBuf> {
    log::info!("Fetching metadata repository...");
    let repo_root = config.metadata.base.join("repo");
    let repo = if !repo_root.exists() {
        log::debug!("Cloning metadata repository from {}", config.metadata.repo);
        RepositoryManager::clone(&config.metadata.repo, repo_root)?
    } else if config.metadata.pull {
        log::debug!(
            "Updating metadata repository at branch: {}",
            config.metadata.branch
        );
        RepositoryManager::pull(repo_root, &config.metadata.branch)?
    } else {
        log::debug!("Loading metadata repository at {}", repo_root.display());
        RepositoryManager::new(repo_root)?
    };
    log::debug!("Generating metadata database...");
    let repo = repo.into_owned_manager()?;
    let database_path = config.metadata.base.join("repo.db");
    repo.to_database(&database_path)?;
    log::info!("Metadata repository fetched.");

    Ok(database_path)
}

fn open_db(p: &str) -> anyhow::Result<RepoDatabaseRead> {
    Ok(RepoDatabaseRead::new(p)?)
}

async fn init_state(config: Config) -> anyhow::Result<web::Data<AppState>> {
    // init metadata
    let database_path = if config.providers.iter().all(|(_, conf)| {
        matches!(
            conf.item,
            ProviderItem::File { strict: true, .. } | ProviderItem::Drive { strict: true, .. }
        )
    }) {
        None
    } else {
        // proxy settings
        if let Some(proxy) = &config.metadata.proxy {
            // if metadata.proxy is an empty string, do not use proxy
            if proxy.is_empty() {
                setup_git2(None);
            } else {
                // otherwise, set proxy in config file
                setup_git2(Some(proxy.clone()));
            }
            // if no proxy was provided, use default behavior (http_proxy)
        }

        Some(init_metadata(&config)?.to_string_lossy().into_owned())
    };

    log::info!("Start initializing providers...");
    let now = SystemTime::now();
    let mut providers = Vec::with_capacity(config.providers.len());
    let mut caches = HashMap::new();

    for (provider_name, provider_config) in config.providers.iter() {
        log::debug!("Initializing provider: {}", provider_name);
        let mut provider: Box<dyn AnniProvider + Send + Sync> = match &provider_config.item {
            ProviderItem::File {
                root,
                strict: false,
                ..
            } => Box::new(
                CommonConventionProvider::new(
                    PathBuf::from(root),
                    open_db(database_path.as_ref().unwrap())?,
                    Box::new(LocalFileSystemProvider),
                )
                .await?,
            ),
            ProviderItem::File {
                root,
                strict: true,
                layer,
            } => Box::new(
                CommonStrictProvider::new(
                    PathBuf::from(root),
                    *layer,
                    Box::new(LocalFileSystemProvider),
                )
                .await?,
            ),
            ProviderItem::Drive {
                drive_id,
                corpora,
                initial_token_path,
                token_path,
                strict,
            } => {
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
                            token_path: token_path.clone(),
                        },
                        if *strict {
                            None
                        } else {
                            Some(open_db(database_path.as_ref().unwrap())?)
                        },
                    )
                    .await?,
                )
            }
        };
        if let Some(cache) = provider_config.cache() {
            log::debug!(
                "Cache configuration detected: root = {}, max-size = {}",
                cache.root(),
                cache.max_size
            );
            if !caches.contains_key(cache.root()) {
                // new cache pool
                let pool = CachePool::new(cache.root(), cache.max_size);
                caches.insert(cache.root().to_string(), Arc::new(pool));
            }
            provider = Box::new(Cache::new(provider, caches[cache.root()].clone()));
        }
        let provider =
            AnnilProvider::new(provider_name.to_string(), provider, provider_config.enable).await?;
        providers.push(provider);
    }
    log::info!(
        "Provider initialization finished, used {:?}",
        now.elapsed().unwrap()
    );

    // etag
    let etag = compute_etag(&providers).await;

    // key
    let key = HS256Key::from_bytes(config.server.key().as_ref());
    let share_key = HS256Key::from_bytes(config.server.share_key().as_ref())
        .with_key_id(config.server.share_key_id());
    let version = format!("Annil v{}", env!("CARGO_PKG_VERSION"));
    let last_update = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    Ok(web::Data::new(AppState {
        providers: RwLock::new(providers),
        key,
        share_key,
        admin_token: config.server.admin_token().to_string(),
        version,
        metadata: config.metadata,
        last_update: RwLock::new(last_update),
        etag: RwLock::new(etag),
    }))
}

#[actix_web::main]
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
    let listen = config.server.listen("0.0.0.0:3614").to_string();
    let state = init_state(config).await?;

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .wrap(AnnilAuth)
            .wrap(
                Cors::default()
                    .allow_any_origin()
                    .allowed_methods(vec!["GET"])
                    .allow_any_header()
                    .send_wildcard(),
            )
            .wrap(Logger::default())
            .service(info)
            .service(admin::reload)
            .service(admin::sign)
            .service(
                web::resource(["/{album_id}/cover", "/{album_id}/{disc_id}/cover"])
                    .route(web::get().to(cover)),
            )
            .service(
                web::resource("/{album_id}/{disc_id}/{track_id}")
                    .route(web::get().to(audio))
                    .route(web::head().to(audio_head)),
            )
            .service(albums)
    })
    .bind(listen)?
    .run()
    .await?;
    Ok(())
}
