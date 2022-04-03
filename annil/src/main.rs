mod provider;
mod config;
mod auth;
mod share;
mod error;
mod services;
mod utils;

use actix_web::{HttpServer, App, web};
use std::sync::Arc;
use anni_provider::providers::{FileBackend, DriveBackend};
use std::path::PathBuf;
use crate::provider::AnnilProvider;
use crate::config::{Config, MetadataConfig, ProviderItem};
use actix_web::middleware::Logger;
use jwt_simple::prelude::HS256Key;
use crate::auth::{AnnilAuth, AnnilClaims};
use anni_provider::{AnniProvider, RepoDatabaseRead};
use std::collections::HashMap;
use anni_provider::cache::{CachePool, Cache};
use anni_provider::providers::drive::DriveProviderSettings;
use actix_cors::Cors;
use crate::error::AnnilError;
use std::time::{SystemTime, UNIX_EPOCH};
use jwt_simple::reexports::serde_json::json;
use parking_lot::RwLock;
use anni_repo::RepositoryManager;
use crate::services::*;
use crate::utils::compute_etag;

pub struct AppState {
    providers: RwLock<Vec<AnnilProvider>>,
    key: HS256Key,
    reload_token: String,

    version: String,
    metadata: MetadataConfig,
    last_update: RwLock<u64>,
    etag: RwLock<String>,
}

async fn init_state(config: Config) -> anyhow::Result<web::Data<AppState>> {
    log::info!("Fetching metadata repository...");
    let repo = RepositoryManager::clone(&config.metadata.repo, config.metadata.base.join("repo"), &config.metadata.branch)?;
    let repo = repo.into_owned_manager()?;
    let database_path = config.metadata.base.join("repo.db");
    repo.to_database(&database_path).await?;
    log::info!("Metadata repository fetched.");

    log::info!("Start initializing providers...");
    let now = SystemTime::now();
    let mut providers = Vec::with_capacity(config.providers.len());
    let mut caches = HashMap::new();

    for (provider_name, provider_config) in config.providers.iter() {
        log::debug!("Initializing provider: {}", provider_name);
        let repo = RepoDatabaseRead::new(database_path.to_string_lossy().as_ref()).await?;
        let mut provider: Box<dyn AnniProvider + Send + Sync> = match &provider_config.item {
            ProviderItem::File { root } =>
                Box::new(FileBackend::new(PathBuf::from(root), repo).await?),
            ProviderItem::Drive { drive_id, corpora, initial_token_path, token_path } => {
                if let Some(initial_token_path) = initial_token_path {
                    if initial_token_path.exists() && !token_path.exists() {
                        let _ = std::fs::copy(initial_token_path, token_path.clone());
                    }
                }
                Box::new(DriveBackend::new(Default::default(), DriveProviderSettings {
                    corpora: corpora.to_string(),
                    drive_id: drive_id.clone(),
                    token_path: token_path.clone(),
                }, repo).await?)
            }
        };
        if let Some(cache) = provider_config.cache() {
            log::debug!("Cache configuration detected: root = {}, max-size = {}", cache.root(), cache.max_size);
            if !caches.contains_key(cache.root()) {
                // new cache pool
                let pool = CachePool::new(cache.root(), cache.max_size);
                caches.insert(cache.root().to_string(), Arc::new(pool));
            }
            provider = Box::new(Cache::new(provider, caches[cache.root()].clone()));
        }
        let provider = AnnilProvider::new(provider_name.to_string(), provider, provider_config.enable).await?;
        providers.push(provider);
    }
    log::info!("Provider initialization finished, used {:?}", now.elapsed().unwrap());

    // etag
    let etag = compute_etag(&providers).await;

    // key
    let key = HS256Key::from_bytes(config.server.key().as_ref());
    let version = format!("Annil v{}", env!("CARGO_PKG_VERSION"));
    let last_update = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    Ok(web::Data::new(AppState {
        providers: RwLock::new(providers),
        key,
        reload_token: config.server.reload_token().to_string(),
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
    let config = Config::from_file(std::env::args().nth(1).unwrap_or_else(|| "config.toml".to_owned()))?;
    let listen = config.server.listen("0.0.0.0:3614").to_string();
    let state = init_state(config).await?;

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .wrap(AnnilAuth)
            .wrap(Cors::default()
                .allow_any_origin()
                .allowed_methods(vec!["GET"])
                .allow_any_header()
                .send_wildcard()
            )
            .wrap(Logger::default())
            .service(info)
            .service(reload)
            .service(
                web::resource([
                    "/{album_id}/cover",
                    "/{album_id}/{disc_id}/cover",
                ])
                    .route(web::get().to(cover))
            )
            .service(web::resource("/{album_id}/{disc_id}/{track_id}")
                .route(web::get().to(audio))
                .route(web::head().to(audio_head))
            )
            .service(albums)
            .service(share::share)
    })
        .bind(listen)?
        .run()
        .await?;
    Ok(())
}
