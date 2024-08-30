use crate::{query::album::Album, schema};

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
    pub update_metadata_tags: Album,
}

#[derive(cynic::InputObject, Debug)]
#[cynic(graphql_type = "MetadataIDInput")]
pub struct MetadataIdinput<'a> {
    pub album: Option<&'a cynic::Id>,
    pub disc: Option<&'a cynic::Id>,
    pub track: Option<&'a cynic::Id>,
}
