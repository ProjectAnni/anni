use std::fmt::{Display, Formatter};
use serde::{Serialize, Deserialize, Serializer, Deserializer};
use toml::Value;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TagRef {
    name: String,
    edition: Option<String>,
}

impl Serialize for TagRef {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer {
        let value = if let Some(edition) = &self.edition {
            if edition.contains(':') {
                let mut table = toml::value::Table::new();
                table.insert("name".to_string(), Value::String(self.name.clone()));
                table.insert("edition".to_string(), Value::String(edition.clone()));
                Value::Table(table)
            } else {
                Value::String(format!("{}:{}", self.name, edition))
            }
        } else {
            Value::String(self.name.clone())
        };

        value.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for TagRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'de> {
        use serde::de;

        let value = Value::deserialize(deserializer)?;
        let result = match value {
            Value::String(tag) => {
                match tag.rsplit_once(':') {
                    Some((name, edition)) => {
                        Self {
                            name: name.to_string(),
                            edition: Some(edition.to_string()),
                        }
                    }
                    None => Self {
                        name: tag,
                        edition: None,
                    }
                }
            }
            Value::Table(table) => {
                let name = table.get("name")
                    .ok_or_else(|| de::Error::custom("Missing field `name`"))?
                    .as_str()
                    .ok_or_else(|| de::Error::custom("Invalid type for field `tag.name`"))?
                    .to_string();
                let edition = match table.get("edition") {
                    Some(edition) => {
                        edition.as_str().map(|r| r.to_string())
                    }
                    None => None,
                };
                Self { name, edition }
            }
            _ => {
                return Err(de::Error::custom("Invalid tag format"));
            }
        };
        Ok(result)
    }
}

impl Display for TagRef {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)?;
        if let Some(edition) = &self.edition {
            write!(f, ":{}", edition)?;
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
pub struct Tag {
    /// Tag name
    name: String,
    /// Tag edition
    edition: Option<String>,
    /// Tag alias
    #[serde(default)]
    alias: Vec<String>,
    /// Tag parents
    #[serde(default)]
    included_by: Vec<TagRef>,
    /// Tag children
    #[serde(default)]
    includes: Vec<TagRef>,
}

impl Tag {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn edition(&self) -> Option<&str> {
        self.edition.as_ref().map(|r| r.as_str())
    }

    pub fn included_by(&self) -> &[TagRef] {
        &self.included_by
    }

    pub fn includes(&self) -> &[TagRef] {
        &self.includes
    }

    pub fn get_ref(&self) -> TagRef {
        TagRef {
            name: self.name.clone(),
            edition: self.edition.clone(),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Tags {
    tag: Vec<Tag>,
}

impl Tags {
    pub fn into_inner(self) -> Vec<Tag> {
        self.tag
    }
}
