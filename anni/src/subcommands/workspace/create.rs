use anni_common::fs;
use anni_workspace::AnniWorkspace;
use clap::Args;
use clap_handler::handler;
use std::env::current_dir;
use std::num::NonZeroU8;
use std::path::PathBuf;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Args, Debug, Clone)]
pub struct WorkspaceCreateAction {
    #[clap(short = 'n', long)]
    name: Option<String>,
    #[clap(short = 'a', long)]
    album_id: Option<String>,
    #[clap(short = 'd', long, default_value = "1")]
    disc_num: NonZeroU8,
    #[clap(short = 'f', long)]
    force: bool,

    path: PathBuf,
}

#[handler(WorkspaceCreateAction)]
fn handle_workspace_create(me: WorkspaceCreateAction) -> anyhow::Result<()> {
    let workspace = AnniWorkspace::find(current_dir()?)?;

    let album_id = me
        .album_id
        .and_then(|a| Uuid::from_str(&a).ok())
        .unwrap_or_else(|| Uuid::new_v4());
    let disc_num = me.disc_num.get();

    // 1. check whether the target path exists
    let user_album_path = match me.name {
        Some(name) => me.path.join(name),
        None => me.path,
    };
    if user_album_path.exists() && !me.force {
        bail!("Target path already exists");
    }

    // 2. create directory in .anni/objects
    let anni_album_path = workspace.get_album_controlled_path(&album_id)?;
    fs::create_dir_all(&anni_album_path)?;

    // 3. create directory in userland
    fs::create_dir_all(&user_album_path)?;
    fs::symlink_dir(&anni_album_path, &user_album_path.join(".album"))?;

    // 4. create disc directories
    if disc_num == 1 {
        // if there's only one disc, it's not necessary to create nested disc directories
    } else {
        // else, more discs, more directories
        for i in 1..=disc_num {
            let disc_path = user_album_path.join(format!("Disc {}", i));
            fs::create_dir_all(&disc_path)?;
        }
    }

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
