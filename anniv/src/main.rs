use actix_web::{HttpServer, App, web, Responder, get, HttpResponse};
use anni_backend::Backend;
use std::sync::Mutex;
use anni_backend::backends::FileBackend;
use std::path::PathBuf;
use actix_web::body::BodyStream;
use std::pin::Pin;
use std::collections::HashMap;

struct AppState {
    backends: Mutex<HashMap<String, Pin<Box<dyn Backend + Send>>>>,
}

#[get("/song/{catalog}/{track_id}/{track_name}")]
async fn song(path: web::Path<(String, u8, String)>, data: web::Data<AppState>) -> impl Responder {
    let (catalog, track_id, track_name) = path.into_inner();
    let backend = data.backends.lock().unwrap();
    if let Some(backend) = backend.get("music") {
        let r = backend.get_audio(&catalog, track_id, &track_name).await.unwrap();
        let stream = tokio_util::io::ReaderStream::new(r);
        HttpResponse::Ok().body(BodyStream::new(stream))
    } else {
        HttpResponse::InternalServerError().finish()
    }
}

async fn init_state() -> web::Data<AppState> {
    let mut backend = FileBackend::new(PathBuf::from("/home/yesterday17/音乐/"));
    backend.update_albums().await.unwrap();
    let mut backends: HashMap<String, Pin<Box<dyn Backend + Send>>> = HashMap::new();
    backends.insert("music".to_owned(), Box::pin(backend));
    web::Data::new(AppState {
        backends: Mutex::new(backends),
    })
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let state = init_state().await;
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
        .await
}
