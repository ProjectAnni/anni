use serde::{Serialize, Deserialize};
use std::str::FromStr;
use std::path::Path;

#[derive(Serialize, Deserialize)]
pub struct Repository {
    repo: RepositoryInner
}

#[derive(Serialize, Deserialize)]
struct RepositoryInner {
    name: String,
    maintainers: Vec<String>,
    edition: String,

    cover: Option<AssetSetting>,
    lyric: Option<AssetSetting>,
}

#[derive(Serialize, Deserialize)]
pub struct AssetSetting {
    pub enable: bool,
    root: Option<String>,
}

impl FromStr for Repository {
    type Err = Box<dyn std::error::Error>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let val: Repository = toml::from_str(s)?;
        Ok(val)
    }
}

impl ToString for Repository {
    fn to_string(&self) -> String {
        toml::to_string(&self).unwrap()
    }
}

impl Repository {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Self {
        Self::from_str(&*std::fs::read_to_string(path.as_ref()).unwrap()).unwrap()
    }

    pub fn name(&self) -> &str {
        self.repo.name.as_ref()
    }

    // https://users.rust-lang.org/t/vec-string-to-str/12619/2
    pub fn maintainers(&self) -> Vec<&str> {
        self.repo.maintainers.iter().map(|x| &**x).collect()
    }

    pub fn edition(&self) -> &str {
        self.repo.edition.as_ref()
    }

    pub fn cover(&self) -> Option<&AssetSetting> {
        self.repo.cover.as_ref()
    }

    pub fn lyric(&self) -> Option<&AssetSetting> {
        self.repo.lyric.as_ref()
    }
}

impl AssetSetting {
    pub fn root(&self) -> Option<&str> {
        self.root.as_ref().map(|r| r.as_ref())
    }
}