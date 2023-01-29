use anni_common::fs::read_to_string;
use directories_next::ProjectDirs;
use once_cell::sync::Lazy;
use serde::de::DeserializeOwned;
use std::path::PathBuf;

static CONFIG_ROOT: Lazy<PathBuf> = Lazy::new(|| init_config());

fn init_config() -> PathBuf {
    let config = std::env::var("ANNI_ROOT").map(PathBuf::from).unwrap_or({
        let dir = ProjectDirs::from("moe", "mmf", "anni").expect("Failed to get project dirs.");
        dir.config_dir().to_path_buf()
    });

    if config.exists() {
        debug!("Config root: {:?}", config);
    } else {
        debug!("Config root does not exist: {:?}", config);
    }
    config
}

pub(crate) fn read_config<T>(name: &'static str) -> anyhow::Result<T>
where
    T: DeserializeOwned,
{
    let file = CONFIG_ROOT.join(format!("{}.toml", name));
    let file = read_to_string(file)?;
    Ok(toml::from_str(&file)?)
}
