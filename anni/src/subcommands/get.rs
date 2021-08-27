use clap::{Clap};

use crate::ll;
use anni_vgmdb::VGMClient;
use futures::executor::block_on;
use crate::cli::Handle;

#[derive(Clap, Debug)]
#[clap(about = ll ! {"get"})]
pub struct GetSubcommand {
    #[clap(subcommand)]
    action: GetAction,
}

impl Handle for GetSubcommand {
    fn handle(&self) -> anyhow::Result<()> {
        self.action.handle()
    }
}

#[derive(Clap, Debug)]
pub enum GetAction {
    #[clap(name = "vgmdb", alias = "vgm")]
    #[clap(about = ll ! {"get-vgmdb"})]
    VGMdb(GetVGMdbAction),
}

impl Handle for GetAction {
    fn handle(&self) -> anyhow::Result<()> {
        match self {
            GetAction::VGMdb(vgmdb) => vgmdb.handle(),
        }
    }
}

#[derive(Clap, Debug)]
pub struct GetVGMdbAction {
    #[clap(short, long)]
    #[clap(about = ll ! ("get-vgmdb-catalog"))]
    catalog: String,
}

impl Handle for GetVGMdbAction {
    fn handle(&self) -> anyhow::Result<()> {
        vgmdb_search(self.catalog.as_str())
    }
}

fn vgmdb_search(catalog: &str) -> anyhow::Result<()> {
    let client = VGMClient::new();
    let album = block_on(client.album(catalog))?;
    println!("[album]");
    println!(r#"title = "{}""#, album.name());
    println!(r#"artist = """#);
    println!(r#"date = {}"#, album.release_date.as_deref().unwrap_or("0000-00-00"));
    println!(r#"type = "normal""#);
    println!(r#"catalog = "{}""#, album.catalog());
    println!();

    for disc in album.discs() {
        println!("[[discs]]");
        println!(r#"catalog = "{}""#, album.catalog());
        println!();

        for track in disc.tracks() {
            println!("[[discs.tracks]]");
            println!(r#"title = "{}""#, track.name());
            println!();
        }
    }
    Ok(())
}