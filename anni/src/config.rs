use std::path::PathBuf;
use directories_next::ProjectDirs;
use anni_common::fs::read_to_string;
use serde::de::DeserializeOwned;

lazy_static::lazy_static! {
    pub static ref CONFIG_ROOT: PathBuf = init_config();
}

fn init_config() -> PathBuf {
    let config = std::env::var("ANNI_ROOT")
        .map(|cfg| PathBuf::from(cfg))
        .unwrap_or({
            let dir = ProjectDirs::from("moe", "mmf", "anni").expect("Failed to get project dirs.");
            dir.config_dir().to_path_buf()
        });

    if config.exists() {
        info!("Config root: {:?}", config);
    } else {
        info!("Config root does not exist: {:?}", config);
    }
    config
}

pub(crate) fn read_config<T>(name: &'static str) -> anyhow::Result<T>
    where T: DeserializeOwned {
    let file = CONFIG_ROOT.join(format!("{}.toml", name));
    let file = read_to_string(file)?;
    Ok(toml::from_str(&file)?)
}