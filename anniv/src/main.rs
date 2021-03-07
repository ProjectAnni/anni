mod backend;
mod config;

use actix_web::{HttpServer, App, web, Responder, get, HttpResponse};
use std::sync::Mutex;
use anni_backend::backends::FileBackend;
use std::path::PathBuf;
use crate::backend::AnnivBackend;
use tokio_util::io::ReaderStream;
use actix_web::http::header::ContentType;
use crate::config::{Config, BackendConfig};

struct AppState {
    backends: Mutex<Vec<AnnivBackend>>,
}

#[get("/albums")]
async fn albums(data: web::Data<AppState>) -> impl Responder {
    let mut albums: Vec<&str> = Vec::new();
    let backends = data.backends.lock().unwrap();
    for backend in backends.iter() {
        let mut a = backend.albums();
        albums.append(&mut a);
    }
    // TODO: Cache album list
    HttpResponse::Ok().json(albums)
}

#[get("/song/{catalog}/{track_id}/{track_name}")]
async fn song(path: web::Path<(String, u8)>, data: web::Data<AppState>) -> impl Responder {
    let (catalog, track_id) = path.into_inner();
    let backend = data.backends.lock().unwrap();
    for backend in backend.iter() {
        if backend.enabled() && backend.has_album(&catalog) {
            let r = backend.get_audio(&catalog, track_id).await.unwrap();
            return HttpResponse::Ok()
                .append_header(("X-Library-Name", backend.name()))
                .content_type(ContentType::octet_stream())
                .streaming(ReaderStream::new(r));
        }
    }
    HttpResponse::NotFound().finish()
}

async fn init_state(configs: &[BackendConfig]) -> anyhow::Result<web::Data<AppState>> {
    let mut backends = Vec::with_capacity(configs.len());
    for config in configs {
        let mut backend: AnnivBackend;
        if config.backend_type == "file" {
            let inner = FileBackend::new(PathBuf::from(config.root()));
            backend = AnnivBackend::new(config.name.to_owned(), Box::new(inner)).await?;
        } else {
            unimplemented!();
        }
        backend.set_enable(config.enable);
        backends.push(backend);
    }
    Ok(web::Data::new(AppState {
        backends: Mutex::new(backends),
    }))
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::from_file("config.toml")?;

    let state = init_state(&config.backends).await?;
    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .service(
                web::scope("/api")
                    .service(song)
                    .service(albums)
            )
    })
        .bind(&config.server.listen("localhost:3614"))?
        .run()
        .await?;
    Ok(())
}
