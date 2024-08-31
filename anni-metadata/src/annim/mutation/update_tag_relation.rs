use crate::annim::{query::album::TagBase, schema};

#[derive(cynic::QueryVariables, Debug)]
pub struct UpdateTagRelationVariables<'a> {
    pub parent: &'a cynic::Id,
    pub remove: bool,
    pub tag: &'a cynic::Id,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    graphql_type = "MetadataMutation",
    variables = "UpdateTagRelationVariables"
)]
pub struct UpdateTagRelation {
    #[arguments(tagId: $tag, parentId: $parent, remove: $remove)]
    pub update_tag_relation: Option<TagRelation>,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct TagRelation {
    pub id: cynic::Id,
    pub tag: TagBase,
    pub parent: TagBase,
}
