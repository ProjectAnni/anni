use crate::library::apply_strict;
use crate::workspace::utils::{
    do_get_album_id, find_dot_anni, get_workspace_album_path, get_workspace_repository_manager,
};
use clap::Args;
use clap_handler::handler;
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
    let dot_anni = find_dot_anni()?;

    let album_id = do_get_album_id(&me.path)?;
    let album_path = get_workspace_album_path(&dot_anni, &album_id).expect("Album path not found");

    if me.tags {
        let repo = get_workspace_repository_manager(&dot_anni)?;
        let repo = repo.into_owned_manager()?;
        let album = repo.album(&album_id.to_string()).expect("Album not found");
        apply_strict(&album_path, album, me.cover)?;
    }

    Ok(())
}
