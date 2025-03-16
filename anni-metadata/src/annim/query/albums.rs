use crate::annim::{schema, Uuid};

use super::album::AlbumFragment;

#[derive(cynic::QueryVariables, Debug)]
pub struct AlbumsVariables {
    pub album_ids: Option<Vec<Uuid>>,
    pub after: Option<String>,
    pub first: Option<i32>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "MetadataQuery", variables = "AlbumsVariables")]
pub struct AlbumsQuery {
    #[arguments(by: { albumIds: $album_ids }, after: $after, first: $first )]
    pub albums: Option<AlbumConnection>,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct AlbumConnection {
    pub page_info: PageInfo,
    pub nodes: Vec<AlbumFragment>,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct PageInfo {
    pub end_cursor: Option<String>,
    pub has_next_page: bool,
    pub has_previous_page: bool,
    pub start_cursor: Option<String>,
}
