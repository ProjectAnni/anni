use crate::library::apply_strict;
use anni_workspace::AnniWorkspace;
use clap::Args;
use clap_handler::handler;
use std::env::current_dir;
use std::path::PathBuf;

#[derive(Args, Debug, Clone)]
pub struct WorkspaceUpdateAction {
    #[clap(short = 't', long)]
    pub tags: bool,

    #[clap(short = 'c', long)]
    pub cover: bool,

    pub path: PathBuf,
}

#[handler(WorkspaceUpdateAction)]
pub async fn handle_workspace_update(me: WorkspaceUpdateAction) -> anyhow::Result<()> {
    let workspace = AnniWorkspace::find(current_dir()?)?;
    let album_id = workspace.get_album_id(&me.path)?;
    let album_path = workspace
        .get_album_controlled_path(&album_id)
        .expect("Album path not found");

    if me.tags {
        let repo = workspace.to_repository_manager()?;
        let repo = repo.into_owned_manager()?;
        let album = repo.album(&album_id).expect("Album not found");
        apply_strict(&album_path, album, me.cover)?;
    }

    Ok(())
}
