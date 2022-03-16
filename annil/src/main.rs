mod backend;
mod config;
mod auth;
mod share;
mod error;
mod services;

use actix_web::{HttpServer, App, web};
use std::sync::Arc;
use anni_provider::providers::{FileBackend, DriveBackend};
use std::path::PathBuf;
use crate::backend::AnnilBackend;
use crate::config::{Config, BackendItem};
use actix_web::middleware::Logger;
use jwt_simple::prelude::HS256Key;
use crate::auth::{AnnilAuth, AnnilClaims};
use anni_provider::{AnniBackend, RepoDatabaseRead};
use std::collections::HashMap;
use anni_provider::cache::{CachePool, Cache};
use anni_provider::providers::drive::DriveBackendSettings;
use actix_cors::Cors;
use crate::error::AnnilError;
use std::time::{SystemTime, UNIX_EPOCH};
use jwt_simple::reexports::serde_json::json;
use tokio::sync::RwLock;
use crate::services::*;

pub struct AppState {
    backends: RwLock<Vec<AnnilBackend>>,
    key: HS256Key,
    reload_token: String,

    version: String,
    last_update: RwLock<u64>,
}

async fn init_state(config: &Config) -> anyhow::Result<web::Data<AppState>> {
    log::info!("Start initializing backends...");
    let now = SystemTime::now();
    let mut backends = Vec::with_capacity(config.backends.len());
    let mut caches = HashMap::new();
    for (backend_name, backend_config) in config.backends.iter() {
        log::debug!("Initializing backend: {}", backend_name);
        let repo = RepoDatabaseRead::new(&backend_config.db.to_string_lossy()).await?;
        let mut backend = match &backend_config.item {
            BackendItem::File { root } =>
                AnniBackend::File(
                    FileBackend::new(
                        PathBuf::from(root),
                        repo,
                    ).await?,
                ),
            BackendItem::Drive { drive_id, corpora, token_path } =>
                AnniBackend::Drive(DriveBackend::new(Default::default(), DriveBackendSettings {
                    corpora: corpora.to_string(),
                    drive_id: drive_id.clone(),
                    token_path: token_path.as_deref().unwrap_or("annil.token").to_string(),
                }, repo).await?),
        };
        if let Some(cache) = backend_config.cache() {
            log::debug!("Cache configuration detected: root = {}, max-size = {}", cache.root(), cache.max_size);
            if !caches.contains_key(cache.root()) {
                // new cache pool
                let pool = CachePool::new(cache.root(), cache.max_size);
                caches.insert(cache.root().to_string(), Arc::new(pool));
            }
            backend = AnniBackend::Cache(Cache::new(backend.into_box(), caches[cache.root()].clone()));
        }
        let backend = AnnilBackend::new(backend_name.to_string(), backend, backend_config.enable).await?;
        backends.push(backend);
    }
    log::info!("Backend initialization finished, used {:?}", now.elapsed().unwrap());

    // key
    let key = HS256Key::from_bytes(config.server.key().as_ref());
    let version = format!("Anniv v{}", env!("CARGO_PKG_VERSION"));
    let last_update = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    Ok(web::Data::new(AppState {
        backends: RwLock::new(backends),
        key,
        reload_token: config.server.reload_token().to_string(),
        version,
        last_update: RwLock::new(last_update),
    }))
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .filter_module("sqlx::query", log::LevelFilter::Warn)
        .init();
    let config = Config::from_file(std::env::args().nth(1).unwrap_or_else(|| "config.toml".to_owned()))?;
    let state = init_state(&config).await?;

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
        .bind(config.server.listen("localhost:3614"))?
        .run()
        .await?;
    Ok(())
}
