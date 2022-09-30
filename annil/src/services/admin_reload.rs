use crate::utils::compute_etag;
use crate::AppState;
use actix_web::{post, web, HttpResponse, Responder};
use anni_repo::RepositoryManager;
use std::time::{SystemTime, UNIX_EPOCH};

#[post("/admin/reload")]
async fn reload(data: web::Data<AppState>) -> impl Responder {
    if let Some(metadata) = &data.metadata {
        if metadata.pull {
            let repo =
                RepositoryManager::pull(metadata.base.join("repo"), &metadata.branch).unwrap();
            let repo = repo.into_owned_manager().unwrap();

            let database_path = metadata.base.join("repo.db");
            repo.to_database(&database_path).unwrap();
        }
    }

    for provider in data.providers.write().iter_mut() {
        if let Err(e) = provider.reload().await {
            log::error!("Failed to reload provider {}: {:?}", provider.name(), e);
        }
    }

    *data.etag.write() = compute_etag(&data.providers.read()).await;
    *data.last_update.write() = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    HttpResponse::Ok().finish()
}
