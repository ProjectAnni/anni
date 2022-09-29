use anni_common::fs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize)]
pub struct WorkspaceConfig {
    #[serde(rename = "workspace")]
    inner: WorkspaceConfigInner,
    #[serde(rename = "library")]
    libraries: HashMap<String, LibraryConfig>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct WorkspaceConfigInner {
    publish_to: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct LibraryConfig {
    pub path: PathBuf,
    pub layers: Option<usize>,
}

impl WorkspaceConfig {
    pub fn new<P>(root: P) -> anyhow::Result<Self>
    where
        P: AsRef<Path>,
    {
        let data = fs::read_to_string(root.as_ref().join("config.toml"))?;
        Ok(toml_edit::easy::from_str(&data)?)
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
