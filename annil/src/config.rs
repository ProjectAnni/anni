use serde::{Serialize, Deserialize};
use std::path::Path;
use std::fs;
use std::collections::HashMap;

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub backends: HashMap<String, BackendConfig>,
}

impl Config {
    pub fn from_file<P: AsRef<Path>>(config_path: P) -> anyhow::Result<Self> {
        let string = fs::read_to_string(config_path)?;
        let result = toml::from_str(&string)?;
        Ok(result)
    }
}

#[derive(Serialize, Deserialize)]
pub struct ServerConfig {
    pub name: String,
    listen: Option<String>,
    #[serde(rename = "hmac-key")]
    key: String,
}

impl ServerConfig {
    pub fn listen(&self, default: &'static str) -> &str {
        if let Some(listen) = &self.listen {
            listen.as_str()
        } else {
            default
        }
    }

    pub fn key(&self) -> &str {
        self.key.as_str()
    }
}

#[derive(Serialize, Deserialize)]
pub struct BackendConfig {
    pub enable: bool,
    #[serde(flatten)]
    pub item: BackendItem,
    cache: Option<CacheConfig>,
}

impl BackendConfig {
    #[inline]
    pub fn cache(&self) -> Option<&CacheConfig> {
        self.cache.as_ref()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum BackendItem {
    #[serde(rename = "file")]
    #[serde(rename_all = "kebab-case")]
    File {
        root: String,
        #[serde(default)]
        strict: bool,
    },
    #[serde(rename = "drive")]
    #[serde(rename_all = "kebab-case")]
    Drive {
        corpora: String,
        drive_id: Option<String>,
        token_path: Option<String>,
    },
}

#[derive(Serialize, Deserialize)]
pub struct CacheConfig {
    root: String,
    #[serde(default, rename = "max-size")]
    pub max_size: usize,
}

impl CacheConfig {
    #[inline]
    pub fn root(&self) -> &str {
        self.root.as_str()
    }
}
