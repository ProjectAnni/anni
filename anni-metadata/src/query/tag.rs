use crate::{schema, DateTime};

#[derive(cynic::QueryVariables, Debug)]
pub struct TagVariables<'a> {
    pub name: &'a str,
    #[cynic(rename = "type")]
    pub type_: Option<TagType>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "MetadataQuery", variables = "TagVariables")]
pub struct TagQuery {
    #[arguments(tagName: $name, tagType: $type_)]
    pub tag: Vec<Tag>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Tag")]
pub struct Tag {
    pub id: cynic::Id,
    pub name: String,
    #[cynic(rename = "type")]
    pub type_: TagType,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub includes: Vec<TagBase>,
    pub included_by: Vec<TagBase>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Tag")]
pub struct TagBase {
    pub id: cynic::Id,
    pub name: String,
    #[cynic(rename = "type")]
    pub type_: TagType,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(cynic::Enum, Clone, Copy, Debug)]
pub enum TagType {
    Artist,
    Group,
    Animation,
    Radio,
    Series,
    Project,
    Game,
    Organization,
    Others,
}
