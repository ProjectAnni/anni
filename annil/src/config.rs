use serde::{Serialize, Deserialize};
use std::path::{Path, PathBuf};
use std::fs;
use std::collections::HashMap;

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    #[serde(rename = "backends")]
    pub providers: HashMap<String, ProviderConfig>,
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
    #[serde(rename = "reload-token")]
    reload_token: String,
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

    pub fn reload_token(&self) -> &str {
        self.reload_token.as_str()
    }
}

#[derive(Serialize, Deserialize)]
pub struct ProviderConfig {
    pub enable: bool,
    pub db: PathBuf,
    #[serde(flatten)]
    pub item: ProviderItem,
    cache: Option<CacheConfig>,
}

impl ProviderConfig {
    #[inline]
    pub fn cache(&self) -> Option<&CacheConfig> {
        self.cache.as_ref()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ProviderItem {
    #[serde(rename = "file")]
    #[serde(rename_all = "kebab-case")]
    File { root: String },
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
