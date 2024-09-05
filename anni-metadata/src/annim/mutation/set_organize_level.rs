use crate::annim::{query::album::MetadataOrganizeLevel, schema};

#[derive(cynic::QueryVariables, Debug)]
pub struct SetMetadataTagsVariables<'a> {
    pub id: &'a cynic::Id,
    pub level: MetadataOrganizeLevel,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    graphql_type = "MetadataMutation",
    variables = "SetMetadataTagsVariables"
)]
pub struct SetMetadataTags {
    #[arguments(input: { id: $id, level: $level })]
    pub update_organize_level: Option<Album>,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct Album {
    pub id: cynic::Id,
}
