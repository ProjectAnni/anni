use std::time::{SystemTime, UNIX_EPOCH};
use actix_web::{HttpResponse, Responder, web, post};
use anni_repo::RepositoryManager;
use crate::AppState;
use crate::utils::compute_etag;

#[post("/admin/reload")]
async fn reload(data: web::Data<AppState>) -> impl Responder {
    if data.metadata.pull {
        let repo = RepositoryManager::pull(data.metadata.base.join("repo"), &data.metadata.branch).unwrap();
        let repo = repo.into_owned_manager().unwrap();

        let database_path = data.metadata.base.join("repo.db");
        repo.to_database(&database_path).unwrap();
    }

    for provider in data.providers.write().iter_mut() {
        if let Err(e) = provider.reload().await {
            log::error!("Failed to reload provider {}: {:?}", provider.name(), e);
        }
    }

    *data.etag.write() = compute_etag(&data.providers.read()).await;
    *data.last_update.write() = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    HttpResponse::Ok().finish()
}