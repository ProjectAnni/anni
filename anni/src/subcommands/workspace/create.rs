use anni_workspace::AnniWorkspace;
use clap::Args;
use clap_handler::handler;
use std::env::current_dir;
use std::num::NonZeroU8;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Args, Debug, Clone)]
pub struct WorkspaceCreateAction {
    #[clap(short = 'a', long)]
    album_id: Option<Uuid>,
    #[clap(short = 'd', long, default_value = "1")]
    disc_num: NonZeroU8,
    #[clap(short = 'f', long)]
    force: bool,

    path: PathBuf,
}

#[handler(WorkspaceCreateAction)]
fn handle_workspace_create(me: WorkspaceCreateAction) -> anyhow::Result<()> {
    let workspace = AnniWorkspace::find(current_dir()?)?;

    let album_id = me.album_id.unwrap_or_else(|| Uuid::new_v4());

    // check whether the target path exists
    let user_album_path = me.path;
    if user_album_path.exists() && !me.force {
        bail!("Target path already exists");
    }

    workspace.create_album(&album_id, &user_album_path, me.disc_num)?;

    Ok(())
}

// #[cfg(test)]
// mod test {
//     use super::{WorkspaceCreateAction};
//     use clap_handler::Handler;
//
//     #[tokio::test]
//     async fn test_create_album() -> anyhow::Result<()> {
//         let path = tempfile::tempdir()?;
//
//         WorkspaceCreateAction {
//             album_id: None,
//             disc_num: 1.into(),
//             path: path.path().to_path_buf(),
//             name: None,
//         }.run().await?;
//
//         Ok(())
//     }
// }
