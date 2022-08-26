use anni_common::fs;
use clap::Args;
use clap_handler::handler;
use std::path::PathBuf;

#[derive(Args, Debug, Clone)]
// #[clap(about = ll!("workspace-init"))]
pub struct WorkspaceInitAction {
    #[clap(long)]
    repo: Option<String>,
    #[clap(long)]
    repo_config: bool,

    path: PathBuf,
}

#[handler(WorkspaceInitAction)]
fn handle_workspace_init(me: WorkspaceInitAction) -> anyhow::Result<()> {
    let dot_anni = me.path.join(".anni");
    if dot_anni.exists() {
        anyhow::bail!("Workspace already exists in {}", dot_anni.display());
    }

    // objects
    fs::create_dir_all(&dot_anni.join("objects"))?;

    // repo
    let repo_path = dot_anni.join("repo");
    if let Some(repo) = me.repo {
        // clone from remote
        anni_repo::RepositoryManager::clone(&repo, &repo_path)?;
    } else {
        // TODO: create new metadata repository
        fs::remove_dir(&dot_anni)?;
        unimplemented!();
    }

    // config.toml
    let config_path = dot_anni.join("config.toml");
    if me.repo_config {
        // symlink .anni/config.toml to .anni/repo/repo.toml to reuse config file
        let repo_config = repo_path.join("repo.toml");
        if !repo_config.exists() {
            anyhow::bail!("Repo config was not found at {}", repo_config.display());
        }

        fs::symlink_file(&repo_config, &config_path)?;
    } else {
        // create a new config file
        fs::write(&config_path, "")?; // TODO: specification of config file
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use super::WorkspaceInitAction;
    use clap_handler::Handler;

    #[tokio::test]
    async fn test_init_clone_workspace() -> anyhow::Result<()> {
        let path = tempfile::tempdir()?;
        let repo = "https://github.com/ProjectAnni/repo";

        WorkspaceInitAction {
            repo: Some(repo.to_string()),
            repo_config: true,
            path: path.path().to_path_buf(),
        }
        .run()
        .await?;

        let dot_anni = path.path().join(".anni");

        assert!(dot_anni.join("objects").exists());
        assert!(dot_anni.join("config.toml").exists());
        assert!(dot_anni.join("repo/repo.toml").exists());
        assert_eq!(
            dot_anni.join("config.toml").read_link()?,
            dot_anni.join("repo/repo.toml")
        );

        Ok(())
    }
}
