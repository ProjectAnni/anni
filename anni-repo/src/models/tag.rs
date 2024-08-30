use crate::error::Error;
use crate::prelude::RepoResult;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::{Borrow, Cow};
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::str::FromStr;
use toml::Value;

/// Simple reference to a tag with its name and edition.
#[derive(Serialize, Deserialize, Clone, Debug, Hash, PartialEq, Eq)]
pub struct TagRef<'a> {
    /// Tag name
    name: Cow<'a, str>,
    /// Tag type
    #[serde(rename = "type")]
    tag_type: TagType,
}

impl<'a> TagRef<'a> {
    pub fn new<N>(name: N, tag_type: TagType) -> Self
    where
        N: Into<Cow<'a, str>>,
    {
        TagRef {
            name: name.into(),
            tag_type,
        }
    }

    pub fn from_cow_str<S>(name: S) -> Self
    where
        S: Into<Cow<'a, str>>,
    {
        let tag: Cow<'a, str> = name.into();
        let (tag_type, name) = tag
            .split_once(':')
            .and_then(|(tag_type, tag_name)| {
                // try to parse tag_type
                let tag_type = TagType::from_str(tag_type);
                match tag_type {
                    // on success, tag type would be extracted
                    Ok(tag_type) => Some((tag_type, Cow::Owned(tag_name.trim().to_string()))),
                    // on failure, DEFAULT would be used
                    Err(_) => None,
                }
            })
            .unwrap_or((TagType::Unknown, tag));
        TagRef { name, tag_type }
    }

    pub fn name(&self) -> &str {
        self.name.deref()
    }

    pub fn tag_type(&self) -> &TagType {
        &self.tag_type
    }

    fn full_clone(&self) -> TagRef<'static> {
        TagRef {
            name: Cow::Owned(self.name.to_string()),
            tag_type: self.tag_type.clone(),
        }
    }
}

impl<'a> TagRef<'a> {
    pub fn into_full(self, parents: Vec<TagString>) -> Tag {
        Tag {
            inner: self.full_clone(),
            names: Default::default(),
            parents,
            children: Vec::new(),
        }
    }
}

impl<'a> Display for TagRef<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.tag_type {
            TagType::Unknown => write!(f, "{}", self.name()),
            ty => write!(f, "{}:{}", ty, self.name()),
        }
    }
}

impl<'tag> Borrow<TagRef<'tag>> for Tag {
    fn borrow(&self) -> &TagRef<'tag> {
        &self.inner
    }
}

impl<'tag> Borrow<TagRef<'tag>> for TagString {
    fn borrow(&self) -> &TagRef<'tag> {
        &self.0
    }
}

/// String representation of a tag
///
/// Formatted by `<edition>:<name>`
///
/// TODO: remove this type
#[derive(Debug, Eq, Clone)]
pub struct TagString(pub(crate) TagRef<'static>);

impl TagString {
    pub fn new(name: String, tag_type: TagType) -> Self {
        Self(TagRef::new(name, tag_type))
    }

    pub fn name(&self) -> &str {
        self.0.name()
    }

    pub fn tag_type(&self) -> &TagType {
        self.0.tag_type()
    }

    pub(crate) fn resolve(
        &mut self,
        tags: &HashMap<String, HashMap<TagType, Tag>>,
    ) -> RepoResult<()> {
        if let TagType::Unknown = self.tag_type {
            if let Some(tags) = tags.get(self.name()) {
                if tags.len() > 1 {
                    return Err(Error::RepoTagDuplicated(self.full_clone()));
                }

                let actual_type = tags.values().next().unwrap().tag_type().clone();
                self.0.tag_type = actual_type;
            }
        }

        Ok(())
    }
}

impl Deref for TagString {
    type Target = TagRef<'static>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Serialize for TagString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        Value::String(format!("{}", self.0)).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for TagString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;

        let value = Value::deserialize(deserializer)?;
        if let Value::String(tag) = value {
            let tag = TagRef::from_cow_str(tag);
            Ok(Self(tag))
        } else {
            Err(Error::custom("Tag must be a string"))
        }
    }
}

impl Display for TagString {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Hash for TagString {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state)
    }
}

impl PartialEq for TagString {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl From<TagRef<'static>> for TagString {
    fn from(value: TagRef<'static>) -> Self {
        Self(value)
    }
}

#[derive(Serialize, Deserialize, Debug, Eq)]
#[serde(deny_unknown_fields)]
pub struct Tag {
    #[serde(flatten)]
    inner: TagRef<'static>,
    /// Tag localized name
    #[serde(default)]
    names: HashMap<String, String>,
    /// Tag parents
    #[serde(default)]
    #[serde(rename = "included-by")]
    parents: Vec<TagString>,
    /// Tag children
    #[serde(default)]
    #[serde(rename = "includes")]
    // TODO: use IndexSet instead
    children: Vec<TagString>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash, Clone)]
#[serde(rename_all = "lowercase")]
pub enum TagType {
    Artist,
    Group,
    Animation,
    Series,
    Project,
    Radio,
    Game,
    Organization,
    Category,
    Unknown,
}

impl AsRef<str> for TagType {
    fn as_ref(&self) -> &str {
        match self {
            TagType::Artist => "artist",
            TagType::Group => "group",
            TagType::Animation => "animation",
            TagType::Series => "series",
            TagType::Project => "project",
            TagType::Radio => "radio",
            TagType::Game => "game",
            TagType::Organization => "organization",
            TagType::Unknown => "unknown",
            TagType::Category => "category",
        }
    }
}

impl FromStr for TagType {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "artist" => Self::Artist,
            "group" => Self::Group,
            "animation" => Self::Animation,
            "series" => Self::Series,
            "project" => Self::Project,
            "radio" => Self::Radio,
            "game" => Self::Game,
            "organization" => Self::Organization,
            "category" => Self::Category,
            "unknown" => Self::Unknown,
            _ => return Err(Error::RepoTagUnknownType(s.to_string())),
        })
    }
}

impl Display for TagType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_ref())
    }
}

impl Hash for Tag {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.hash(state)
    }
}

impl PartialEq for Tag {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Display for Tag {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

impl Tag {
    pub fn name(&self) -> &str {
        self.inner.name()
    }

    pub fn names(&self) -> &HashMap<String, String> {
        &self.names
    }

    pub fn tag_type(&self) -> &TagType {
        self.inner.tag_type()
    }

    pub fn parents<'me, 'tag>(&'me self) -> impl Iterator<Item = &'me TagRef<'tag>>
    where
        'tag: 'me,
    {
        self.parents.iter().map(|i| &i.0)
    }

    pub fn simple_children<'me, 'tag>(&'me self) -> impl Iterator<Item = &'me TagRef<'tag>>
    where
        'tag: 'me,
    {
        self.children.iter().map(|i| &i.0)
    }

    pub fn get_owned_ref(&self) -> TagRef<'static> {
        TagRef {
            name: self.inner.name.clone(),
            tag_type: self.inner.tag_type.clone(),
        }
    }
}

impl AsRef<TagRef<'static>> for Tag {
    fn as_ref(&self) -> &TagRef<'static> {
        &self.inner
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

#[cfg(test)]
mod tests {
    use crate::models::{TagRef, TagString, TagType};

    #[test]
    fn test_tag_string_serialize() {
        let tag = TagString(TagRef::new("Test", TagType::Artist));
        assert_eq!(tag.to_string(), "artist:Test".to_string());
    }

    #[test]
    fn test_tag_string_deserialize() {
        #[derive(serde::Deserialize)]
        struct TestStruct {
            tags: Vec<TagString>,
        }

        let TestStruct { tags } = toml::from_str(
            r#"
tags = [
  "artist:123",
  "group:456",
  "implicit-tag-type",
  "implicit:tag-type with :",
]
"#,
        )
        .unwrap();
        assert_eq!(tags.len(), 4);

        assert_eq!(tags[0].name, "123");
        assert_eq!(tags[0].tag_type, TagType::Artist);

        assert_eq!(tags[1].name, "456");
        assert_eq!(tags[1].tag_type, TagType::Group);

        assert_eq!(tags[2].name, "implicit-tag-type");
        assert_eq!(tags[2].tag_type, TagType::Unknown);

        assert_eq!(tags[3].name, "implicit:tag-type with :");
        assert_eq!(tags[3].tag_type, TagType::Unknown);
    }
}
