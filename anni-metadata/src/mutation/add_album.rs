use crate::query::album::{Album, TrackType};
use crate::schema;

#[derive(cynic::QueryVariables, Debug)]
pub struct AddAlbumVariables<'a> {
    pub album: AddAlbumInput<'a>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "MetadataMutation", variables = "AddAlbumVariables")]
pub struct AddAlbum {
    #[arguments(input: $album)]
    pub add_album: Option<Album>,
}

#[derive(cynic::InputObject, Debug)]
pub struct AddAlbumInput<'a> {
    pub album_id: Option<crate::Uuid>,
    pub title: &'a str,
    pub edition: Option<&'a str>,
    pub catalog: Option<&'a str>,
    pub artist: &'a str,
    pub year: i32,
    pub month: Option<i32>,
    pub day: Option<i32>,
    pub extra: Option<crate::Json>,
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
    pub type_: TrackType,
}
