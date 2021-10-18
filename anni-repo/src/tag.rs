use serde::{Serialize, Deserialize, Serializer, Deserializer};
use toml::Value;

#[derive(Clone, Debug)]
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
    included_by: Vec<String>,
    /// Tag children
    #[serde(default)]
    includes: Vec<String>,
}