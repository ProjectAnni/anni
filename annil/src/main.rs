mod backend;
mod config;
mod db;
mod auth;
mod share;

use actix_web::{HttpServer, App, web, Responder, get, HttpResponse, HttpRequest};
use std::sync::Mutex;
use anni_backend::backends::FileBackend;
use std::path::PathBuf;
use crate::backend::AnnivBackend;
use tokio_util::io::ReaderStream;
use actix_web::http::header::ContentType;
use crate::config::Config;
use sqlx::{Pool, Postgres};
use sqlx::postgres::PgPoolOptions;
use actix_web::middleware::Logger;
use jwt_simple::prelude::HS256Key;
use crate::auth::CanFetch;

struct AppState {
    backends: Mutex<Vec<AnnivBackend>>,
    pool: Pool<Postgres>,
    key: HS256Key,
}

/// Get available albums of current annil server
#[get("/albums")]
async fn albums(data: web::Data<AppState>) -> impl Responder {
    let mut albums: Vec<&str> = Vec::new();
    let backends = data.backends.lock().unwrap();
    for backend in backends.iter() {
        let mut a = backend.albums();
        albums.append(&mut a);
    }
    HttpResponse::Ok().json(albums)
}

/// Get audio in an album with {catalog} and {track_id}
#[get("/{catalog}/{track_id}")]
async fn audio(req: HttpRequest, path: web::Path<(String, u8)>, data: web::Data<AppState>) -> impl Responder {
    let validator = match auth::auth_user_or_share(&req, &data.key) {
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
            let r = backend.get_audio(&catalog, track_id).await.unwrap();
            return HttpResponse::Ok()
                .append_header(("X-Backend-Name", backend.name()))
                .content_type(ContentType::octet_stream())
                .streaming(ReaderStream::new(r));
        }
    }
    HttpResponse::NotFound().finish()
}

/// Get audio cover of an album with {catalog}
#[get("/{catalog}/cover")]
async fn cover(req: HttpRequest, path: web::Path<String>, data: web::Data<AppState>) -> impl Responder {
    let validator = match auth::auth_user_or_share(&req, &data.key) {
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
                        .content_type(ContentType("audio/flac".parse().unwrap()))// TODO: store MIME in backend
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
        let mut backend: AnnivBackend;
        if backend_config.backend_type == "file" {
            let inner = FileBackend::new(PathBuf::from(backend_config.root()));
            backend = AnnivBackend::new(backend_config.name.to_owned(), Box::new(inner)).await?;
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
    let key = HS256Key::from_bytes(config.server.token().as_ref());
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
            .service(audio)
            .service(albums)
            .service(cover)
            .service(share::share)
    })
        .bind(config.server.listen("localhost:3614"))?
        .run()
        .await?;
    Ok(())
}
