use clap::Parser;

use crate::ll;
use anni_vgmdb::VGMClient;
use futures::executor::block_on;
use anni_derive::Handler;

#[derive(Parser, Handler, Debug)]
#[clap(about = ll ! {"get"})]
pub struct GetSubcommand {
    #[clap(subcommand)]
    action: GetAction,
}

#[derive(Parser, Handler, Debug)]
pub enum GetAction {
    #[clap(name = "vgmdb", alias = "vgm")]
    #[clap(about = ll ! {"get-vgmdb"})]
    VGMdb(GetVGMdbAction),
}

#[derive(Parser, Handler, Debug)]
#[clap_handler(get_vgmdb)]
pub struct GetVGMdbAction {
    #[clap(short = 'H', long = "host", default_value = "https://vgmdb.info/")]
    #[clap(about = ll ! {"vgmdb-api-host"})]
    api_host: String,

    #[clap(short, long)]
    #[clap(about = ll ! {"get-vgmdb-catalog"})]
    catalog: String,
}

fn get_vgmdb(me: &GetVGMdbAction) -> anyhow::Result<()> {
    let client = VGMClient::new(me.api_host.clone());
    let album = block_on(client.album(&me.catalog))?;
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
