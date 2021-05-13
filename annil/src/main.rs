mod backend;
mod config;
mod auth;
mod share;

use actix_web::{HttpServer, App, web, Responder, get, HttpResponse, HttpRequest};
use std::sync::{Mutex, Arc};
use anni_backend::backends::{FileBackend, DriveBackend};
use std::path::PathBuf;
use crate::backend::AnnilBackend;
use tokio_util::io::ReaderStream;
use crate::config::{Config, BackendItem};
use actix_web::middleware::Logger;
use jwt_simple::prelude::HS256Key;
use crate::auth::CanFetch;
use anni_backend::AnniBackend;
use std::collections::{HashSet, HashMap};
use anni_backend::cache::{CachePool, Cache};
use anni_backend::backends::drive::DriveBackendSettings;

struct AppState {
    backends: Mutex<Vec<AnnilBackend>>,
    key: HS256Key,
}

/// Get available albums of current annil server
#[get("/albums")]
async fn albums(data: web::Data<AppState>) -> impl Responder {
    let mut result: HashSet<&str> = HashSet::new();
    let backends = data.backends.lock().unwrap();
    for backend in backends.iter() {
        let albums = backend.albums();
        result.extend(albums.iter());
    }
    HttpResponse::Ok().json(result)
}

/// Get audio in an album with {catalog} and {track_id}
#[get("/{catalog}/{track_id}")]
async fn audio(req: HttpRequest, path: web::Path<(String, u8)>, data: web::Data<AppState>) -> impl Responder {
    let validator = match auth::auth_user_or_share(&req, &data.key).await {
        Some(r) => r,
        None => return HttpResponse::Unauthorized().finish(),
    };
    let (catalog, track_id) = path.into_inner();
    if !validator.can_fetch(&catalog, Some(track_id)) {
        return HttpResponse::Forbidden().finish();
    }

    let backends = data.backends.lock().unwrap();
    for backend in backends.iter() {
        if backend.enabled() && backend.has_album(&catalog) {
            let audio = backend.get_audio(&catalog, track_id).await.unwrap();
            return HttpResponse::Ok()
                .append_header(("X-Origin-Type", format!("audio/{}", audio.extension)))
                .append_header(("X-Origin-Size", audio.size))
                .content_type(format!("audio/{}", audio.extension))
                .streaming(ReaderStream::new(audio.reader));
        }
    }
    HttpResponse::NotFound().finish()
}

/// Get audio cover of an album with {catalog}
#[get("/{catalog}/cover")]
async fn cover(req: HttpRequest, path: web::Path<String>, data: web::Data<AppState>) -> impl Responder {
    let validator = match auth::auth_user_or_share(&req, &data.key).await {
        Some(r) => r,
        None => return HttpResponse::Unauthorized().finish(),
    };
    let catalog = path.into_inner();
    if !validator.can_fetch(&catalog, None) {
        return HttpResponse::Forbidden().finish();
    }

    let backends = data.backends.lock().unwrap();
    for backend in backends.iter() {
        if backend.enabled() && backend.has_album(&catalog) {
            return match backend.get_cover(&catalog).await {
                Ok(cover) => {
                    HttpResponse::Ok()
                        .content_type("image/jpeg")
                        .streaming(ReaderStream::new(cover))
                }
                Err(_) => {
                    HttpResponse::NotFound().finish()
                }
            };
        }
    }
    HttpResponse::NotFound().finish()
}

async fn init_state(config: &Config) -> anyhow::Result<web::Data<AppState>> {
    log::info!("Start initializing backends...");
    let now = std::time::SystemTime::now();
    let mut backends = Vec::with_capacity(config.backends.len());
    let mut caches = HashMap::new();
    for (backend_name, backend_config) in config.backends.iter() {
        log::debug!("Initializing backend: {}", backend_name);
        let mut backend = match &backend_config.item {
            BackendItem::File { root, strict } =>
                AnniBackend::File(FileBackend::new(PathBuf::from(root), *strict)),
            BackendItem::Drive { drive_id, corpora, token_path } =>
                AnniBackend::Drive(DriveBackend::new(Default::default(), DriveBackendSettings {
                    corpora: corpora.to_string(),
                    drive_id: drive_id.clone(),
                    token_path: token_path.as_deref().unwrap_or("annil.token").to_string(),
                }).await?),
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
        let mut backend = AnnilBackend::new(backend_name.to_string(), backend).await?;
        backend.set_enable(backend_config.enable);
        backends.push(backend);
    }
    log::info!("Backend initialization finished, used {:?}", now.elapsed().unwrap());

    // key
    let key = HS256Key::from_bytes(config.server.key().as_ref());
    Ok(web::Data::new(AppState {
        backends: Mutex::new(backends),
        key,
    }))
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let config = Config::from_file(std::env::args().nth(1).unwrap_or("config.toml".to_owned()))?;
    let state = init_state(&config).await?;

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .wrap(Logger::default())
            .service(cover)
            .service(audio)
            .service(albums)
            .service(share::share)
    })
        .bind(config.server.listen("localhost:3614"))?
        .run()
        .await?;
    Ok(())
}
