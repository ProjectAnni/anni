use std::iter::Map;
use clap::{App, ArgMatches};
use crate::subcommands::flac::FlacSubcommand;
use crate::subcommands::cue::CueSubcommand;
use crate::subcommands::split::SplitSubcommand;
use crate::subcommands::convention::ConventionSubcommand;
use crate::subcommands::repo::RepoSubcommand;
use std::collections::HashMap;
use std::collections::hash_map::Values;
use crate::subcommands::get::GetSubcommand;

mod split;
mod convention;
mod cue;
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
        result.add_subcommand(Box::new(FlacSubcommand));
        result.add_subcommand(Box::new(CueSubcommand));
        result.add_subcommand(Box::new(SplitSubcommand));
        result.add_subcommand(Box::new(ConventionSubcommand));
        result.add_subcommand(Box::new(RepoSubcommand));
        result.add_subcommand(Box::new(GetSubcommand));
        result
    }
}

impl Subcommands {
    fn add_subcommand(&mut self, cmd: Box<dyn Subcommand>) {
        self.subcommands.insert(cmd.name(), cmd);
    }

    pub fn iter(&self) -> Map<Values<'_, &'static str, Box<dyn Subcommand>>, fn(&Box<dyn Subcommand>) -> App<'static>> {
        self.subcommands.values().map(|r| r.create())
    }

    pub fn handle(&self, subcommand: &str, matches: &ArgMatches) -> anyhow::Result<()> {
        self.subcommands.get(subcommand).unwrap().handle(matches)
    }
}