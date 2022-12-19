use crate::error::Error;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::{Borrow, Cow};
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::str::FromStr;
use toml_edit::easy::Value;

/// Simple reference to a tag with its name and edition.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct TagRef<'a> {
    /// Tag name
    name: Cow<'a, str>,
    /// Tag type
    #[serde(rename = "type")]
    tag_type: TagType,
}

impl<'a> TagRef<'a> {
    pub fn new(name: Cow<'a, str>, tag_type: TagType) -> Self {
        TagRef { name, tag_type }
    }

    pub fn simple(name: Cow<'a, str>) -> Self {
        TagRef {
            name,
            tag_type: TagType::Default,
        }
    }

    pub fn name(&self) -> &str {
        self.name.deref()
    }

    pub fn tag_type(&self) -> &TagType {
        &self.tag_type
    }

    pub fn full_clone(&self) -> TagRef<'static> {
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
        write!(f, "{}:{}", self.tag_type(), self.name())
    }
}

impl<'a> Hash for TagRef<'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(self.name.as_bytes());
        // TODO: write tag type
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
#[derive(Debug, PartialEq, Eq)]
pub struct TagString(pub(crate) TagRef<'static>);

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
        let name = self.0.name();
        Value::String(format!("{}:{name}", self.0.tag_type)).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for TagString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de;
        use serde::de::Error;

        let value = Value::deserialize(deserializer)?;
        if let Value::String(tag) = value {
            let (tag_type, tag_name) = tag
                .split_once(":")
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .map(|(a, b)| (Some(TagType::from_str(&a)), b))
                .unwrap_or_else(|| (None, tag));
            let tag_type = tag_type
                .unwrap_or(Ok(TagType::Default))
                .map_err(|e| D::Error::custom(e))?;
            Ok(Self(TagRef {
                name: tag_name.into(),
                tag_type,
            }))
        } else {
            Err(de::Error::custom("Tag must be a string"))
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

impl From<TagRef<'static>> for TagString {
    fn from(value: TagRef<'static>) -> Self {
        Self(value)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
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

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
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
    Default,
    Category,
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
            "default" => Self::Default,
            "category" => Self::Category,
            _ => return Err(Error::RepoTagUnknownType(s.to_string())),
        })
    }
}

impl Display for TagType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            TagType::Artist => "artist",
            TagType::Group => "group",
            TagType::Animation => "animation",
            TagType::Series => "series",
            TagType::Project => "project",
            TagType::Radio => "radio",
            TagType::Game => "game",
            TagType::Organization => "organization",
            TagType::Default => "default",
            TagType::Category => "category",
        })
    }
}

impl Hash for Tag {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.hash(state)
    }
}

impl Display for Tag {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

impl Tag {
    pub fn name(&self) -> &str {
        &self.inner.name()
    }

    pub fn names(&self) -> impl Iterator<Item = (&String, &String)> {
        self.names.iter()
    }

    pub fn tag_type(&self) -> &TagType {
        &self.inner.tag_type()
    }

    pub fn parents(&self) -> &[TagString] {
        &self.parents
    }

    pub fn children_simple<'me, 'tag>(&'me self) -> impl Iterator<Item = &'me TagRef<'tag>>
    where
        'tag: 'me,
    {
        self.children.iter().map(|i| &i.0)
    }

    pub fn get_ref(&self) -> &TagRef {
        &self.inner
    }

    pub fn get_owned_ref(&self) -> TagRef<'static> {
        TagRef {
            name: self.inner.name.clone(),
            tag_type: self.inner.tag_type.clone(),
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

#[cfg(test)]
mod tests {
    use crate::models::{TagRef, TagString, TagType};

    #[test]
    fn test_tag_string_serialize() {
        let tag = TagString(TagRef::new("Test".into(), TagType::Artist));
        assert_eq!(tag.to_string(), "artist:Test".to_string());
    }

    #[test]
    fn test_tag_string_deserialize() {
        #[derive(serde::Deserialize)]
        struct TestStruct {
            tags: Vec<TagString>,
        }

        let TestStruct { tags } = toml_edit::easy::from_str(
            r#"
tags = [
  "artist:123",
  "group:456",
]
"#,
        )
        .unwrap();
        assert_eq!(tags.len(), 2);

        assert_eq!(tags[0].name, "123");
        assert_eq!(tags[0].tag_type, TagType::Artist);

        assert_eq!(tags[1].name, "456");
        assert_eq!(tags[1].tag_type, TagType::Group);
    }
}
