use serde::{Serialize, Deserialize};
use std::path::Path;
use std::fs;

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub backends: Vec<BackendConfig>,
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
    pub db: String,
    #[serde(rename = "hmac-token")]
    token: String,
}

impl ServerConfig {
    pub fn listen(&self, default: &'static str) -> &str {
        if let Some(listen) = &self.listen {
            listen.as_str()
        } else {
            default
        }
    }

    pub fn token(&self) -> &str {
        self.token.as_str()
    }
}

#[derive(Serialize, Deserialize)]
pub struct BackendConfig {
    pub name: String,
    pub enable: bool,
    #[serde(rename = "type")]
    pub backend_type: String,
    root: Option<String>,
}

impl BackendConfig {
    pub fn root(&self) -> &str {
        if let Some(root) = &self.root {
            root.as_str()
        } else {
            panic!("no root provided!")
        }
    }
}