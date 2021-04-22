mod backend;
mod config;
mod auth;
mod share;

use actix_web::{HttpServer, App, web, Responder, get, HttpResponse, HttpRequest};
use std::sync::Mutex;
use anni_backend::backends::FileBackend;
use std::path::PathBuf;
use crate::backend::AnnilBackend;
use tokio_util::io::ReaderStream;
use crate::config::Config;
use actix_web::middleware::Logger;
use jwt_simple::prelude::HS256Key;
use crate::auth::CanFetch;
use anni_backend::AnniBackend;
use std::collections::HashSet;

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
                        .append_header(("X-Backend-Name", backend.name()))
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
    for backend_config in config.backends.iter() {
        let mut backend: AnnilBackend;
        if backend_config.backend_type == "file" {
            log::debug!("Initializing backend: {}", backend_config.name);
            let inner = FileBackend::new(PathBuf::from(backend_config.root()), backend_config.strict);
            backend = AnnilBackend::new(backend_config.name.to_owned(), AnniBackend::File(inner)).await?;
        } else {
            unimplemented!();
        }
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
