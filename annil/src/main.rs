mod backend;
mod config;
mod db;
mod auth;
mod share;

use actix_web::{HttpServer, App, web, Responder, get, HttpResponse, HttpRequest};
use std::sync::Mutex;
use anni_backend::backends::{FileBackend, StrictFileBackend};
use std::path::PathBuf;
use crate::backend::AnnilBackend;
use tokio_util::io::ReaderStream;
use crate::config::Config;
use sqlx::{Pool, Postgres};
use sqlx::postgres::PgPoolOptions;
use actix_web::middleware::Logger;
use jwt_simple::prelude::HS256Key;
use crate::auth::CanFetch;
use anni_backend::AnniBackend;
use std::borrow::Cow;

struct AppState {
    backends: Mutex<Vec<AnnilBackend>>,
    pool: Pool<Postgres>,
    key: HS256Key,
}

/// Get available albums of current annil server
#[get("/albums")]
async fn albums(data: web::Data<AppState>) -> impl Responder {
    let mut albums: Vec<Cow<str>> = Vec::new();
    let backends = data.backends.lock().unwrap();
    for backend in backends.iter() {
        let mut a = backend.albums().await;
        albums.append(&mut a);
    }
    HttpResponse::Ok().json(albums)
}

/// Get audio in an album with {catalog} and {track_id}
#[get("/{catalog}/{track_id}")]
async fn audio(req: HttpRequest, path: web::Path<(String, u8)>, data: web::Data<AppState>) -> impl Responder {
    let validator = match auth::auth_user_or_share(&req, &data.key, data.pool.clone()).await {
        Some(r) => r,
        None => return HttpResponse::Unauthorized().finish(),
    };
    let (catalog, track_id) = path.into_inner();
    if !validator.can_fetch(&catalog, Some(track_id)) {
        return HttpResponse::Forbidden().finish();
    }

    let backends = data.backends.lock().unwrap();
    for backend in backends.iter() {
        if backend.enabled() && backend.has_album(&catalog).await {
            let r = backend.get_audio(&catalog, track_id).await.unwrap();
            return HttpResponse::Ok()
                .append_header(("X-Backend-Name", backend.name()))
                .content_type("audio/flac")// TODO: store MIME in backend
                .streaming(ReaderStream::new(r));
        }
    }
    HttpResponse::NotFound().finish()
}

/// Get audio cover of an album with {catalog}
#[get("/{catalog}/cover")]
async fn cover(req: HttpRequest, path: web::Path<String>, data: web::Data<AppState>) -> impl Responder {
    let validator = match auth::auth_user_or_share(&req, &data.key, data.pool.clone()).await {
        Some(r) => r,
        None => return HttpResponse::Unauthorized().finish(),
    };
    let catalog = path.into_inner();
    if !validator.can_fetch(&catalog, None) {
        return HttpResponse::Forbidden().finish();
    }

    let backends = data.backends.lock().unwrap();
    for backend in backends.iter() {
        if backend.enabled() && backend.has_album(&catalog).await {
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
    let mut backends = Vec::with_capacity(config.backends.len());
    for backend_config in config.backends.iter() {
        let mut backend: AnnilBackend;
        if backend_config.backend_type == "file" {
            let inner = FileBackend::new(PathBuf::from(backend_config.root()));
            backend = AnnilBackend::new(backend_config.name.to_owned(), AnniBackend::File(inner)).await?;
        } else if backend_config.backend_type == "file_strict" {
            let inner = StrictFileBackend::new(PathBuf::from(backend_config.root()));
            backend = AnnilBackend::new(backend_config.name.to_owned(), AnniBackend::StrictFile(inner)).await?;
        } else {
            unimplemented!();
        }
        backend.set_enable(backend_config.enable);
        backends.push(backend);
    }

    // database
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.server.db).await?;

    // key
    let key = HS256Key::from_bytes(config.server.key().as_ref());
    Ok(web::Data::new(AppState {
        backends: Mutex::new(backends),
        pool,
        key,
    }))
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    std::env::set_var("RUST_LOG", "actix_web=info");
    env_logger::init();
    let config = Config::from_file("config.toml")?;
    let state = init_state(&config).await?;

    // Init db
    db::init_db(state.pool.clone()).await?;

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
