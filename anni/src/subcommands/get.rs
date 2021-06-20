use clap::{ArgMatches, App, Arg};

use crate::subcommands::Subcommand;
use crate::i18n::ClapI18n;
use anni_vgmdb::VGMClient;
use futures::executor::block_on;

pub(crate) struct GetSubcommand;

impl Subcommand for GetSubcommand {
    fn name(&self) -> &'static str {
        "get"
    }

    fn create(&self) -> App<'static> {
        App::new("get")
            .about_ll("get")
            .subcommand(App::new("vgmdb")
                .alias("vgm")
                .about_ll("get-vgmdb")
                .arg(Arg::new("catalog")
                    .about_ll("get-vgmdb-catalog")
                    .long("catalog")
                    .short('c')
                    .takes_value(true)
                )
            )
    }

    fn handle(&self, matches: &ArgMatches) -> anyhow::Result<()> {
        if let Some(matches) = matches.subcommand_matches("vgmdb") {
            let catalog = matches.value_of("catalog").expect("catalog not provided");
            vgmdb_search(catalog)?;
        }
        Ok(())
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