use crate::annim::{query::album::AlbumFragment, schema};

#[derive(cynic::QueryVariables, Debug)]
pub struct SetMetadataTagsVariables<'a> {
    pub tags: Vec<&'a cynic::Id>,
    pub target: MetadataIdinput<'a>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    graphql_type = "MetadataMutation",
    variables = "SetMetadataTagsVariables"
)]
pub struct SetMetadataTags {
    #[arguments(input: $target, tags: $tags)]
    pub update_metadata_tags: AlbumFragment,
}

#[derive(cynic::InputObject, Debug)]
#[cynic(graphql_type = "MetadataIDInput")]
pub struct MetadataIdinput<'a> {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub album: Option<&'a cynic::Id>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub disc: Option<&'a cynic::Id>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub track: Option<&'a cynic::Id>,
}
