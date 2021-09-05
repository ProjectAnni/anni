use clap::Clap;

use crate::ll;
use anni_vgmdb::VGMClient;
use futures::executor::block_on;
use anni_derive::ClapHandler;
use anni_common::traits::Handle;

#[derive(Clap, ClapHandler, Debug)]
#[clap(about = ll ! {"get"})]
pub struct GetSubcommand {
    #[clap(subcommand)]
    action: GetAction,
}

#[derive(Clap, ClapHandler, Debug)]
pub enum GetAction {
    #[clap(name = "vgmdb", alias = "vgm")]
    #[clap(about = ll ! {"get-vgmdb"})]
    VGMdb(GetVGMdbAction),
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