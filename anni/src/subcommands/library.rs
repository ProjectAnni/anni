use std::path::PathBuf;
use clap::Parser;
use anni_clap_handler::{Handler, handler};
use anni_common::fs;
use crate::ll;

#[derive(Parser, Debug, Clone, Handler)]
#[clap(about = ll ! ("library"))]
#[clap(alias = "lib")]
pub struct LibrarySubcommand {
    #[clap(subcommand)]
    action: LibraryAction,
}

#[derive(Parser, Debug, Clone, Handler)]
pub enum LibraryAction {
    New(LibraryNewAlbumAction),
}

#[derive(Parser, Debug, Clone)]
pub struct LibraryNewAlbumAction {
    #[clap(short = 'n', long, default_value = "1")]
    disc_num: u8,

    #[clap(default_value = ".")]
    path: PathBuf,
}

#[handler(LibraryNewAlbumAction)]
pub fn library_new_album(me: &LibraryNewAlbumAction) -> anyhow::Result<()> {
    if me.disc_num == 0 {
        anyhow::bail!("disc_num must be > 0");
    }

    let album_id = uuid::Uuid::new_v4().to_string();
    let album_path = me.path.join(album_id);
    fs::create_dir(&album_path)?;

    for i in 1..=me.disc_num {
        let disc_path = album_path.join(format!("{}", i));
        fs::create_dir(&disc_path)?;
    }

    Ok(())
}
