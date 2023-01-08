use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize, Clone)]
pub struct MetadataConfig {
    pub repo: String,
    pub branch: String,
    pub base: PathBuf,
    #[serde(default = "default_true")]
    pub pull: bool,
    pub proxy: Option<String>,
}

fn default_true() -> bool {
    true
}

#[cfg(feature = "metadata")]
impl MetadataConfig {
    pub fn init(&self) -> anyhow::Result<PathBuf> {
        use anni_repo::RepositoryManager;

        log::info!("Fetching metadata repository...");

        let repo_root = self.base.join("repo");
        let repo = if !repo_root.exists() {
            log::debug!("Cloning metadata repository from {}", self.repo);
            RepositoryManager::clone(&self.repo, repo_root)?
        } else if self.pull {
            log::debug!("Updating metadata repository at branch: {}", self.branch);
            RepositoryManager::pull(repo_root, &self.branch)?
        } else {
            log::debug!("Loading metadata repository at {}", repo_root.display());
            RepositoryManager::new(repo_root)?
        };

        log::debug!("Generating metadata database...");
        let repo = repo.into_owned_manager()?;
        let database_path = self.base.join("repo.db");
        repo.to_database(&database_path)?;

        log::info!("Metadata repository fetched.");
        Ok(database_path)
    }

    pub fn into_db(self) -> LazyDb {
        use anni_repo::setup_git2;
        // proxy settings
        if let Some(proxy) = &self.proxy {
            // if metadata.proxy is an empty string, do not use proxy
            if proxy.is_empty() {
                setup_git2(None);
            } else {
                // otherwise, set proxy in config file
                setup_git2(Some(proxy.clone()));
            }
            // if no proxy was provided, use default behavior (http_proxy)
        }

        LazyDb {
            metadata: self,
            db_path: None,
        }
    }
}

#[cfg(feature = "metadata")]
pub struct LazyDb {
    metadata: MetadataConfig,
    db_path: Option<PathBuf>,
}

#[cfg(feature = "metadata")]
impl LazyDb {
    pub fn open(&mut self) -> anyhow::Result<anni_repo::db::RepoDatabaseRead> {
        let db = match self.db_path {
            Some(ref p) => p,
            None => {
                let p = self.metadata.init()?;
                self.db_path.insert(p)
            }
        };
        Ok(anni_repo::db::RepoDatabaseRead::new(db)?)
    }
}
