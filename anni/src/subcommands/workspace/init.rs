use crate::ll;
use anni_common::fs;
use anni_metadata::AnnimClient;
use anni_workspace::AnniWorkspace;
use clap::Args;
use clap_handler::handler;
use std::path::PathBuf;

#[derive(Args, Debug, Clone)]
#[clap(about = ll!("workspace-init"))]
pub struct WorkspaceInitAction {
    #[clap(long)]
    remote: Option<String>,
    #[clap(long)]
    auth: Option<String>,

    #[clap(long)]
    repo: Option<String>,

    path: PathBuf,
}

#[handler(WorkspaceInitAction)]
fn handle_workspace_init(me: WorkspaceInitAction) -> anyhow::Result<()> {
    // TODO: move init logic to anni-workspace
    let dot_anni = me.path.join(".anni");
    if dot_anni.exists() {
        anyhow::bail!("Workspace already exists in {}", dot_anni.display());
    }

    // SAFETY: Only path-related methods would be called;
    let workspace = unsafe { AnniWorkspace::new_unchecked(dot_anni) };

    // objects
    fs::create_dir_all(workspace.objects_root())?;

    // metadata
    let config_path = workspace.config_path();
    match (me.remote, me.repo) {
        (Some(endpoint), None) => {
            let client = AnnimClient::new(endpoint.to_string(), me.auth.as_deref());
            // TODO: print server info

            let mut config_content = format!(
                r#"
[workspace.metadata]
type = "remote"
endpoint = "{endpoint}"
"#
            );
            if let Some(auth) = me.auth {
                config_content += &format!(r#"token = "{auth}""#);
            }
            // config
            fs::write(config_path, config_content.trim())?;
        }
        (None, Some(repo)) => {
            let repo_path = workspace.repo_root();
            anni_repo::RepositoryManager::clone(&repo, &repo_path)?;

            // config
            fs::write(
                config_path,
                r#"
[workspace.metadata]
type = "repo"
"#
                .trim(),
            )?;
        }
        (a, _) => {
            // SAFETY: the workspace is just created and no actual content exists in it.
            unsafe {
                workspace.destroy()?;
            }
            if a.is_some() {
                anyhow::bail!("Cannot specify both remote and repo");
            }
        }
    }

    eprintln!(
        "Initialized empty Anni workspace in {}",
        workspace.workspace_root().display()
    );
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
            path: path.path().to_path_buf(),
            remote: None,
            auth: None,
        }
        .run()
        .await?;

        let dot_anni = path.path().join(".anni");

        assert!(dot_anni.join("objects").exists());
        assert!(dot_anni.join("config.toml").exists());
        assert!(dot_anni.join("repo/repo.toml").exists());

        Ok(())
    }
}
