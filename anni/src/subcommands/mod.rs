use std::iter::Map;
use clap::{App, ArgMatches, AppSettings};
use crate::subcommands::flac::FlacSubcommand;
use crate::subcommands::split::SplitSubcommand;
use crate::subcommands::convention::ConventionSubcommand;
use crate::subcommands::repo::RepoSubcommand;
use std::collections::HashMap;
use std::collections::hash_map::Values;
use crate::subcommands::get::GetSubcommand;

mod split;
mod convention;
mod flac;
mod repo;
mod get;

pub trait Subcommand {
    fn name(&self) -> &'static str;
    fn create(&self) -> clap::App<'static>;
    fn handle(&self, matches: &clap::ArgMatches) -> anyhow::Result<()>;
}

pub struct Subcommands {
    subcommands: HashMap<&'static str, Box<dyn Subcommand>>,
}

impl Default for Subcommands {
    fn default() -> Self {
        let mut result = Self {
            subcommands: Default::default(),
        };
        result.add_subcommand(FlacSubcommand);
        result.add_subcommand(SplitSubcommand);
        result.add_subcommand(ConventionSubcommand);
        result.add_subcommand(RepoSubcommand);
        result.add_subcommand(GetSubcommand);
        result
    }
}

impl Subcommands {
    fn add_subcommand(&mut self, cmd: impl Subcommand + 'static) {
        self.subcommands.insert(cmd.name(), Box::new(cmd));
    }

    pub fn iter(&self) -> Map<Values<'_, &'static str, Box<dyn Subcommand>>, fn(&Box<dyn Subcommand>) -> App<'static>> {
        self.subcommands.values().map(|r| r.create()
            .setting(AppSettings::ArgRequiredElseHelp))
    }

    pub fn handle(&self, subcommand: &str, matches: &ArgMatches) -> anyhow::Result<()> {
        self.subcommands.get(subcommand).unwrap().handle(matches)
    }
}