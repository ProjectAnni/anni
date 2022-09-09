use crate::workspace::utils::*;
use anni_common::fs;
use anyhow::bail;
use clap::Args;
use clap_handler::handler;
use std::path::PathBuf;

#[derive(Args, Debug, Clone)]
pub struct WorkspacePublishAction {
    // publish_to: Option<PathBuf>,
    path: Vec<PathBuf>,
}

#[handler(WorkspacePublishAction)]
pub fn handle_workspace_publish(me: WorkspacePublishAction) -> anyhow::Result<()> {
    let root = find_dot_anni()?;
    let config = super::config::WorkspaceConfig::new(&root)?;

    let publish_to = config
        .publish_to()
        .expect("Target audio library is not specified in workspace config file.");

    for path in me.path {
        if let Some(layers) = publish_to.layers {
            // publish as strict
            unimplemented!()
        } else {
            // publish as convention
        }
    }
    Ok(())
}
