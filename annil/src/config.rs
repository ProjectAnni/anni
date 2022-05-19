use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::fs;
use std::collections::HashMap;

#[derive(Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub metadata: MetadataConfig,
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

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ServerConfig {
    /// Server name
    pub name: String,
    /// Port to listen on
    listen: Option<String>,
    /// HMAC key for JWT
    #[serde(rename = "hmac-key")]
    key: String,
    share_key: String,
    share_key_id: String,
    /// Password to reload data
    admin_token: String,
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

    pub fn share_key(&self) -> &str {
        self.share_key.as_str()
    }

    pub fn share_key_id(&self) -> &str {
        self.share_key_id.as_str()
    }

    pub fn admin_token(&self) -> &str {
        self.admin_token.as_str()
    }
}

#[derive(Deserialize)]
pub struct MetadataConfig {
    pub repo: String,
    pub branch: String,
    pub base: PathBuf,
    #[serde(default = "default_true")]
    pub pull: bool,
    pub proxy: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
pub struct ProviderConfig {
    pub enable: bool,
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

#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum ProviderItem {
    #[serde(rename = "file")]
    #[serde(rename_all = "kebab-case")]
    File { root: String, strict: bool },
    #[serde(rename = "drive")]
    #[serde(rename_all = "kebab-case")]
    Drive {
        corpora: String,
        drive_id: Option<String>,
        initial_token_path: Option<PathBuf>,
        token_path: PathBuf,
    },
}

#[derive(Deserialize)]
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
