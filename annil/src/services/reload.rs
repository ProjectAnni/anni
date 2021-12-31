use std::time::{SystemTime, UNIX_EPOCH};
use actix_web::{HttpResponse, Responder, web, post};
use crate::AppState;

#[post("/reload")]
async fn reload(data: web::Data<AppState>) -> impl Responder {
    for backend in data.backends.write().await.iter_mut() {
        if let Err(e) = backend.reload().await {
            log::error!("Failed to reload backend {}: {:?}", backend.name(), e);
        }
    }
    *data.last_update.write().await = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    HttpResponse::Ok().finish()
}