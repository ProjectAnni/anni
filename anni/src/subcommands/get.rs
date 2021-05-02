use clap::{ArgMatches, App, Arg};

use crate::subcommands::Subcommand;
use crate::i18n::ClapI18n;

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
        Ok(())
    }
}

fn vgmdb_search() {
    //
}