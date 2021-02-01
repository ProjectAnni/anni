use serde::{Serialize, Deserialize};
use std::error::Error;
use std::str::FromStr;

#[derive(Deserialize)]
struct RepositoryDeserializeWrapper {
    repo: Repository,
}

#[derive(Serialize)]
struct RepositorySerializeWrapper<'a> {
    repo: &'a Repository,
}

#[derive(Serialize, Deserialize)]
pub struct Repository {
    name: String,
    version: String,
    authors: Vec<String>,
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
    type Err = Box<dyn Error>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let val: RepositoryDeserializeWrapper = toml::from_str(s)?;
        Ok(val.repo)
    }
}

impl ToString for Repository {
    fn to_string(&self) -> String {
        toml::to_string_pretty(&RepositorySerializeWrapper { repo: self }).unwrap()
    }
}

impl Repository {
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    pub fn version(&self) -> &str {
        self.version.as_ref()
    }

    // https://users.rust-lang.org/t/vec-string-to-str/12619/2
    pub fn authors(&self) -> Vec<&str> {
        self.authors.iter().map(|x| &**x).collect()
    }

    pub fn edition(&self) -> &str {
        self.edition.as_ref()
    }

    pub fn cover(&self) -> Option<&AssetSetting> {
        self.cover.as_ref()
    }

    pub fn lyric(&self) -> Option<&AssetSetting> {
        self.lyric.as_ref()
    }
}

impl AssetSetting {
    pub fn root(&self) -> Option<&str> {
        self.root.as_ref().map(|r| r.as_ref())
    }
}