use clap::{ArgMatches, App};
use crate::subcommands::Subcommand;
use crate::fl;

pub(crate) struct ConventionSubcommand;

impl Subcommand for ConventionSubcommand {
    fn name(&self) -> &'static str {
        "convention"
    }

    fn create(&self) -> App<'static> {
        App::new("convention")
            .about(fl!("convention"))
            .alias("conv")
            .subcommand(App::new("check")
                .about(fl!("convention-check"))
            )
    }

    fn handle(&self, matches: &ArgMatches) -> anyhow::Result<()> {
        todo!()
    }
}
