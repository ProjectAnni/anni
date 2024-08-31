use crate::annim::query::album::{AlbumFragment, TrackTypeInput};
use crate::annim::{schema, Json, Uuid};

#[derive(cynic::QueryVariables, Debug)]
pub struct AddAlbumVariables<'a> {
    pub album: AddAlbumInput<'a>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "MetadataMutation", variables = "AddAlbumVariables")]
pub struct AddAlbumMutation {
    #[arguments(input: $album)]
    pub add_album: AlbumFragment,
}

#[derive(cynic::InputObject, Debug)]
pub struct AddAlbumInput<'a> {
    pub album_id: Option<Uuid>,
    pub title: &'a str,
    pub edition: Option<&'a str>,
    pub catalog: Option<&'a str>,
    pub artist: &'a str,
    pub year: i32,
    pub month: Option<i32>,
    pub day: Option<i32>,
    pub extra: Option<Json>,
    pub discs: Vec<CreateAlbumDiscInput<'a>>,
}

#[derive(cynic::InputObject, Debug)]
pub struct CreateAlbumDiscInput<'a> {
    pub title: Option<&'a str>,
    pub catalog: Option<&'a str>,
    pub artist: Option<&'a str>,
    pub tracks: Vec<CreateAlbumTrackInput<'a>>,
}

#[derive(cynic::InputObject, Debug)]
pub struct CreateAlbumTrackInput<'a> {
    pub title: &'a str,
    pub artist: &'a str,
    #[cynic(rename = "type")]
    pub type_: TrackTypeInput,
}
