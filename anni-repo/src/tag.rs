use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};
use serde::{Serialize, Deserialize, Serializer, Deserializer};
use toml::Value;

/// RepoTag is a wrapper type for the actual tag used in anni metadata repository.
/// All part of code other than serialize/deserialize part should use this type
/// instead of the underlying tag types.
#[derive(Debug, Eq)]
pub enum RepoTag {
    Ref(TagRef),
    Full(Tag),
}

/// Hash implementation fo RepoTag depends on the underlying tag type.
impl Hash for RepoTag {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            RepoTag::Ref(ref tag) => tag.hash(state),
            RepoTag::Full(ref tag) => tag.hash(state),
        }
    }
}

/// Two RepoTags equal iff their name and edition are the same.
impl PartialEq for RepoTag {
    fn eq(&self, other: &Self) -> bool {
        let (name_a, edition_a) = match self {
            RepoTag::Ref(r) => (r.name.as_str(), r.edition.as_deref()),
            RepoTag::Full(f) => (f.name.as_str(), f.edition.as_deref()),
        };
        let (name_b, edition_b) = match other {
            RepoTag::Ref(r) => (r.name.as_str(), r.edition.as_deref()),
            RepoTag::Full(f) => (f.name.as_str(), f.edition.as_deref()),
        };
        name_a.eq(name_b) && edition_a.eq(&edition_b)
    }
}

/// Clone a TagRef for corresponding RepoTag.
impl Clone for RepoTag {
    fn clone(&self) -> Self {
        RepoTag::Ref(self.get_ref())
    }
}

impl Display for RepoTag {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RepoTag::Ref(r) => write!(f, "{}", r),
            RepoTag::Full(t) => write!(f, "{}", t),
        }
    }
}

impl RepoTag {
    /// Get an owned TagRef of the RepoTag.
    pub fn get_ref(&self) -> TagRef {
        match self {
            RepoTag::Ref(r) => r.clone(),
            RepoTag::Full(t) => t.get_ref(),
        }
    }
}

/// Simple reference to a tag with owned name and edition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagRef {
    name: String,
    edition: Option<String>,
}

impl TagRef {
    /// Extend a simple TagRef to a full tag with name, edition and parents.
    pub fn extend_simple(self, parents: Vec<TagRef>) -> Tag {
        Tag {
            name: self.name,
            edition: self.edition,
            alias: vec![],
            parents,
            children: vec![],
        }
    }
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
                            edition: if edition.is_empty() { None } else { Some(edition.to_string()) },
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

impl Hash for TagRef {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(self.name.as_bytes());
        if let Some(edition) = &self.edition {
            state.write(b":");
            state.write(edition.as_bytes());
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
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
    #[serde(rename = "included-by")]
    parents: Vec<TagRef>,
    /// Tag children
    #[serde(default)]
    #[serde(rename = "includes")]
    children: Vec<TagRef>,
}

impl Hash for Tag {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(self.name.as_bytes());
        if let Some(edition) = &self.edition {
            state.write(b":");
            state.write(edition.as_bytes());
        }
    }
}

impl Display for Tag {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.get_ref())
    }
}

impl Tag {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn edition(&self) -> Option<&str> {
        self.edition.as_ref().map(|r| r.as_str())
    }

    pub fn parents(&self) -> &[TagRef] {
        &self.parents
    }

    #[doc(hidden)]
    pub fn is_empty(&self) -> bool {
        self.name.is_empty() && self.edition.is_none()
    }

    #[doc(hidden)]
    pub fn children_raw(&self) -> &[TagRef] {
        &self.children
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
