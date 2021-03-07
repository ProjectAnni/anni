mod backend;

use actix_web::{HttpServer, App, web, Responder, get, HttpResponse};
use std::sync::Mutex;
use anni_backend::backends::FileBackend;
use std::path::PathBuf;
use crate::backend::AnnivBackend;
use tokio_util::io::ReaderStream;

struct AppState {
    backends: Mutex<Vec<AnnivBackend>>,
}

#[get("/song/{catalog}/{track_id}/{track_name}")]
async fn song(path: web::Path<(String, u8, String)>, data: web::Data<AppState>) -> impl Responder {
    let (catalog, track_id, track_name) = path.into_inner();
    let backend = data.backends.lock().unwrap();
    for backend in backend.iter() {
        if backend.enabled() && backend.has_album(&catalog) {
            let r = backend.get_audio(&catalog, track_id, &track_name).await.unwrap();
            return HttpResponse::Ok().streaming(ReaderStream::new(r));
        }
    }
    HttpResponse::InternalServerError().finish()
}

async fn init_state() -> anyhow::Result<web::Data<AppState>> {
    let backend = FileBackend::new(PathBuf::from("/home/yesterday17/音乐/"));
    let backend = AnnivBackend::new("default".to_owned(), Box::new(backend)).await?;
    Ok(web::Data::new(AppState {
        backends: Mutex::new(vec![backend]),
    }))
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    let state = init_state().await?;
    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .service(
                web::scope("/api")
                    .service(song)
            )
    })
        .bind("127.0.0.1:2222")?
        .run()
        .await?;
    Ok(())
}
