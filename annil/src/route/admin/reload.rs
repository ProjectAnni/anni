use crate::utils::compute_etag;
use crate::AppState;
use anni_repo::RepositoryManager;
use axum::Extension;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

pub async fn reload(Extension(data): Extension<Arc<AppState>>) {
    if let Some(metadata) = &data.metadata {
        if metadata.pull {
            let repo =
                RepositoryManager::pull(metadata.base.join("repo"), &metadata.branch).unwrap();
            let repo = repo.into_owned_manager().unwrap();

            let database_path = metadata.base.join("repo.db");
            repo.to_database(&database_path).unwrap();
        }
    }

    for provider in data.providers.write().await.iter_mut() {
        if let Err(e) = provider.reload().await {
            log::error!("Failed to reload provider {}: {:?}", provider.name(), e);
        }
    }

    *data.etag.write().await = Some(compute_etag(&data.providers.read().await).await);
    *data.last_update.write().await = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
}
