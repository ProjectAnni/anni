use crate::provider::AnnilProvider;
use crate::state::AnnilState;
use anni_provider::AnniProvider;
use axum::Extension;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

pub async fn reload<P>(
    Extension(data): Extension<Arc<AnnilState>>,
    Extension(provider): Extension<Arc<AnnilProvider<P>>>,
) where
    P: AnniProvider + Send + Sync,
{
    #[cfg(feature = "metadata")]
    if let Some(metadata) = &data.metadata {
        use anni_repo::RepositoryManager;

        if metadata.pull {
            let repo =
                RepositoryManager::pull(metadata.base.join("repo"), &metadata.branch).unwrap();
            let repo = repo.into_owned_manager().unwrap();

            let database_path = metadata.base.join("repo.db");
            repo.to_database(&database_path).unwrap();
        }
    }

    if let Err(e) = provider.write().await.reload().await {
        log::error!("Failed to reload provider: {:?}", e);
    }

    *data.etag.write().await = provider.compute_etag().await.unwrap();
    *data.last_update.write().await = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
}
