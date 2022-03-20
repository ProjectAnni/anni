use std::time::{SystemTime, UNIX_EPOCH};
use actix_web::{HttpResponse, Responder, web, post};
use anni_repo::RepositoryManager;
use crate::AppState;

#[post("/reload")]
async fn reload(data: web::Data<AppState>) -> impl Responder {
    let repo = RepositoryManager::clone(&data.metadata.repo, data.metadata.base.join("repo"), &data.metadata.branch).unwrap();
    let repo = repo.to_owned_manager().unwrap();

    let database_path = data.metadata.base.join("repo.db");
    repo.to_database(&database_path).await.unwrap();

    for provider in data.providers.write().iter_mut() {
        if let Err(e) = provider.reload().await {
            log::error!("Failed to reload provider {}: {:?}", provider.name(), e);
        }
    }
    *data.last_update.write() = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    HttpResponse::Ok().finish()
}