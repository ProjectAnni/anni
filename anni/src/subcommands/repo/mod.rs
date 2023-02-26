mod add;
mod get;
mod lint;
mod print;
mod watch;

use crate::args::ActionFile;
use crate::{ball, fl, ll};
use add::*;
use anni_workspace::AnniWorkspace;
use lint::*;
use print::*;
use watch::*;

use anni_repo::library::{file_name, AlbumFolderInfo};
use anni_repo::prelude::*;
use anni_repo::RepositoryManager;
use clap::{Args, Subcommand, ValueEnum};
use clap_handler::{handler, Context, Handler};
use get::RepoGetAction;
use std::env::current_dir;
use std::io::Read;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Args, Debug, Clone, Handler)]
#[clap(about = ll!("repo"))]
#[handler_inject(repo_fields)]
pub struct RepoSubcommand {
    #[clap(long, env = "ANNI_REPO")]
    #[clap(help = ll!("repo-root"))]
    root: Option<PathBuf>,

    #[clap(subcommand)]
    action: RepoAction,
}

impl RepoSubcommand {
    fn repo_root(&self) -> PathBuf {
        match &self.root {
            Some(root) => root.clone(),
            None => {
                let workspace =
                    AnniWorkspace::find(current_dir().unwrap()).expect("Workspace not found");
                workspace.repo_root()
            }
        }
    }

    async fn repo_fields(&self, ctx: &mut Context) -> anyhow::Result<()> {
        let manager = RepositoryManager::new(self.repo_root())?;
        ctx.insert(manager);
        Ok(())
    }
}

#[derive(Subcommand, Handler, Debug, Clone)]
pub enum RepoAction {
    #[clap(about = ll!("repo-clone"))]
    Clone(RepoCloneAction),
    #[clap(about = ll!("repo-add"))]
    Add(RepoAddAction),
    #[clap(about = ll!("repo-import"))]
    Import(RepoImportAction),
    #[clap(about = ll!("repo-get"))]
    Get(RepoGetAction),
    #[clap(about = ll!("repo-edit"))]
    Edit(RepoEditAction),
    #[clap(about = ll!("repo-lint"))]
    Lint(RepoLintAction),
    #[clap(about = ll!("repo-print"))]
    Print(RepoPrintAction),
    #[clap(name = "db")]
    #[clap(about = ll!("repo-db"))]
    Database(RepoDatabaseAction),
    Watch(RepoWatchAction),
}

#[derive(Args, Debug, Clone)]
pub struct RepoCloneAction {
    #[clap(required = true)]
    url: String,
    root: Option<PathBuf>,
}

#[handler(RepoCloneAction)]
fn repo_clone(me: RepoCloneAction) -> anyhow::Result<()> {
    let root = me.root.unwrap_or_else(|| PathBuf::from(".")).join("repo");
    log::info!(
        "{}",
        fl!("repo-clone-start", path = root.display().to_string())
    );
    RepositoryManager::clone(&me.url, root)?;
    log::info!("{}", fl!("repo-clone-done"));
    Ok(())
}

#[derive(Args, Debug, Clone)]
pub struct RepoImportAction {
    #[clap(short = 'D', long = "duplicate")]
    allow_duplicate: bool,

    #[clap(value_enum)]
    #[clap(short = 'f', long, default_value = "toml")]
    format: RepoImportFormat,

    file: ActionFile,
}

#[derive(ValueEnum, Debug, Clone)]
pub enum RepoImportFormat {
    // Json,
    Toml,
}

#[handler(RepoImportAction)]
fn repo_import(me: &RepoImportAction, manager: &RepositoryManager) -> anyhow::Result<()> {
    let mut reader = me.file.to_reader()?;
    let mut result = String::new();
    reader.read_to_string(&mut result)?;

    match me.format {
        RepoImportFormat::Toml => {
            let album = Album::from_str(&result)?;
            manager.add_album(album, me.allow_duplicate)?;
        }
    }
    Ok(())
}

#[derive(Args, Debug, Clone)]
pub struct RepoEditAction {
    #[clap(required = true)]
    directories: Vec<PathBuf>,
}

#[handler(RepoEditAction)]
fn repo_edit(me: &RepoEditAction, manager: &RepositoryManager) -> anyhow::Result<()> {
    fn do_edit(directory: &PathBuf, manager: &RepositoryManager) -> anyhow::Result<()> {
        let last = file_name(directory)?;
        debug!(target: "repo|edit", "Directory: {}", last);
        if !is_album_folder(&last) {
            ball!("repo-invalid-album", name = last);
        }

        let AlbumFolderInfo { catalog, .. } = AlbumFolderInfo::from_str(&last)?;
        debug!(target: "repo|edit", "Catalog: {}", catalog);
        for file in manager.album_paths(&catalog)? {
            edit::edit_file(&file)?;
        }
        Ok(())
    }

    for directory in me.directories.iter() {
        if let Err(e) = do_edit(directory, manager) {
            error!("{}", e);
        }
    }
    Ok(())
}

fn is_album_folder(input: &str) -> bool {
    let bytes = input.as_bytes();
    let second_last_byte = bytes[bytes.len() - 2];
    !(bytes[bytes.len() - 1] == b']' && second_last_byte > b'0' && second_last_byte < b'9')
}

////////////////////////////////////////////////////////////////////////
// Repo database
#[derive(Args, Debug, Clone)]
pub struct RepoDatabaseAction {
    #[clap(help = ll!("export-to"))]
    output: PathBuf,
}

#[handler(RepoDatabaseAction)]
fn repo_database_action(me: RepoDatabaseAction, manager: RepositoryManager) -> anyhow::Result<()> {
    if !me.output.is_dir() {
        bail!("Output path must be a directory!");
    }

    let manager = manager.into_owned_manager()?;
    manager.to_database(&me.output.join("repo.db"))?;

    Ok(())
}
