use serde::{Serialize, Deserialize};
use anni_derive::FromFile;
use anni_common::traits::FromFile;
use std::path::Path;
use std::str::FromStr;

#[derive(Serialize, Deserialize, FromFile, Debug)]
pub struct Category {
    #[serde(rename = "category")]
    info: CategoryInfo,
    subcategory: Vec<SubCategory>,
}

impl FromStr for Category {
    type Err = crate::Error;

    fn from_str(toml_str: &str) -> Result<Self, Self::Err> {
        let category = toml::from_str(toml_str)
            .map_err(|e| crate::Error::TomlParseError {
                target: "Category",
                err: e,
            })?;

        Ok(category)
    }
}

impl Category {
    pub fn info(&self) -> &CategoryInfo {
        &self.info
    }

    pub fn subcategories(&self) -> impl Iterator<Item=&SubCategory> {
        self.subcategory.iter()
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CategoryInfo {
    name: String,
    #[serde(rename = "type")]
    category_type: CategoryType,
    albums: Vec<String>,
}

impl CategoryInfo {
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub fn category_type(&self) -> CategoryType {
        self.category_type.clone()
    }

    pub fn albums(&self) -> impl Iterator<Item=&str> {
        self.albums.iter().map(|a| a.as_str())
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SubCategory {
    name: String,
    albums: Vec<String>,
}

impl SubCategory {
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub fn albums(&self) -> impl Iterator<Item=&str> {
        self.albums.iter().map(|a| a.as_str())
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum CategoryType {
    #[serde(rename = "A")]
    Artist,
    #[serde(rename = "AAAA")]
    Group,
    #[serde(rename = "Bangumi")]
    Bangumi,
    #[serde(rename = "Game")]
    Game,
}
