use core::slice;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};
use toml_edit::easy::Value;

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
        let name_a = match self {
            RepoTag::Ref(r) => r.name(),
            RepoTag::Full(f) => f.name.as_str(),
        };
        let name_b = match other {
            RepoTag::Ref(r) => r.name(),
            RepoTag::Full(f) => f.name.as_str(),
        };
        name_a.eq(name_b)
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
    pub unsafe fn from_ref(tag: &TagRef) -> Self {
        let ptr = tag.0.as_ptr();
        let tag = String::from_utf8_unchecked(slice::from_raw_parts(ptr, tag.0.len()).to_vec());
        RepoTag::Ref(TagRef(tag))
    }

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
pub struct TagRef(String);

impl TagRef {
    pub fn new(name: String) -> Self {
        TagRef(name)
    }

    pub fn name(&self) -> &str {
        self.0.as_str()
    }

    /// Extend a simple TagRef to a full tag with name, edition and parents.
    pub fn extend_simple(self, parents: Vec<TagRef>, tag_type: TagType) -> Tag {
        Tag {
            name: self.0,
            alias: vec![],
            tag_type,
            parents,
            children: Default::default(),
        }
    }
}

impl Serialize for TagRef {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        Value::String(self.0.clone()).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for TagRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de;

        let value = Value::deserialize(deserializer)?;
        if let Value::String(tag) = value {
            Ok(Self(tag))
        } else {
            Err(de::Error::custom("Tag should be a string"))
        }
    }
}

impl Display for TagRef {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Hash for TagRef {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(self.0.as_bytes());
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Tag {
    /// Tag name
    name: String,
    /// Tag alias
    #[serde(default)]
    alias: Vec<String>,
    #[serde(rename = "type")]
    tag_type: TagType,
    /// Tag parents
    #[serde(default)]
    #[serde(rename = "included-by")]
    parents: Vec<TagRef>,
    /// Tag children
    #[serde(default)]
    #[serde(rename = "includes")]
    children: TagChildren,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TagType {
    Artist,
    Group,
    Animation,
    Series,
    Project,
    Game,
    Organization,
    Default,
    Category,
}

impl ToString for TagType {
    fn to_string(&self) -> String {
        match self {
            TagType::Artist => "artist".to_string(),
            TagType::Group => "group".to_string(),
            TagType::Animation => "animation".to_string(),
            TagType::Series => "series".to_string(),
            TagType::Project => "project".to_string(),
            TagType::Game => "game".to_string(),
            TagType::Organization => "organization".to_string(),
            TagType::Default => "default".to_string(),
            TagType::Category => "category".to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct TagChildren {
    #[serde(default)]
    artist: Vec<TagRef>,
    #[serde(default)]
    group: Vec<TagRef>,
    #[serde(default)]
    animation: Vec<TagRef>,
    #[serde(default)]
    series: Vec<TagRef>,
    #[serde(default)]
    project: Vec<TagRef>,
    #[serde(default)]
    game: Vec<TagRef>,
    #[serde(default)]
    organization: Vec<TagRef>,
    #[serde(default)]
    default: Vec<TagRef>,
    #[serde(default)]
    category: Vec<TagRef>,
}

impl Hash for Tag {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(self.name.as_bytes());
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

    pub fn tag_type(&self) -> &TagType {
        &self.tag_type
    }

    pub fn alias(&self) -> &[String] {
        &self.alias
    }

    pub fn parents(&self) -> &[TagRef] {
        &self.parents
    }

    pub fn children_simple(&self) -> impl Iterator<Item = (&TagRef, TagType)> {
        self.children
            .artist
            .iter()
            .map(|t| (t, TagType::Artist))
            .chain(self.children.group.iter().map(|t| (t, TagType::Group)))
            .chain(
                self.children
                    .animation
                    .iter()
                    .map(|t| (t, TagType::Animation)),
            )
            .chain(self.children.series.iter().map(|t| (t, TagType::Series)))
            .chain(self.children.project.iter().map(|t| (t, TagType::Project)))
            .chain(self.children.game.iter().map(|t| (t, TagType::Game)))
            .chain(
                self.children
                    .organization
                    .iter()
                    .map(|t| (t, TagType::Organization)),
            )
            .chain(self.children.default.iter().map(|t| (t, TagType::Default)))
            .chain(
                self.children
                    .category
                    .iter()
                    .map(|t| (t, TagType::Category)),
            )
    }

    pub fn get_ref(&self) -> TagRef {
        TagRef(self.name.clone())
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
