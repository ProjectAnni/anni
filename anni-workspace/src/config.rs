use crate::WorkspaceError;
use anni_common::fs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceConfig {
    #[serde(rename = "workspace")]
    inner: WorkspaceConfigInner,
    #[serde(rename = "library")]
    libraries: HashMap<String, LibraryConfig>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[serde(deny_unknown_fields)]
pub struct WorkspaceConfigInner {
    publish_to: Option<String>,
    metadata: Option<WorkspaceMetadata>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum WorkspaceMetadata {
    Repo,
    Remote {
        endpoint: String,
        token: Option<String>,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LibraryConfig {
    pub path: PathBuf,
    pub layers: Option<usize>,
}

impl WorkspaceConfig {
    pub fn new<P>(root: P) -> Result<Self, WorkspaceError>
    where
        P: AsRef<Path>,
    {
        let data = fs::read_to_string(root.as_ref().join("config.toml"))?;
        Ok(toml::from_str(&data)?)
    }

    pub fn metadata(&self) -> WorkspaceMetadata {
        self.inner
            .metadata
            .clone()
            .unwrap_or(WorkspaceMetadata::Repo)
    }

    pub fn publish_to(&self) -> Option<&LibraryConfig> {
        self.inner
            .publish_to
            .as_ref()
            .and_then(|p| self.libraries.get(p))
    }

    #[allow(dead_code)]
    pub fn get_library(&self, name: &str) -> Option<&LibraryConfig> {
        self.libraries.get(name)
    }
}
