use crate::schema;

#[derive(cynic::QueryVariables, Debug)]
pub struct AlbumVariables {
    pub album_id: crate::Uuid,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "MetadataQuery", variables = "AlbumVariables")]
pub struct AlbumQuery {
    #[arguments(albumId: $album_id)]
    pub album: Option<Album>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Album")]
pub struct Album {
    pub id: cynic::Id,
    pub album_id: crate::Uuid,
    pub level: MetadataOrganizeLevel,
    pub title: String,
    pub edition: Option<String>,
    pub catalog: Option<String>,
    pub artist: String,
    pub year: i32,
    pub month: Option<i32>,
    pub day: Option<i32>,
    pub created_at: crate::DateTime,
    pub updated_at: crate::DateTime,
    pub discs: Vec<Disc>,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct Disc {
    pub id: cynic::Id,
    pub index: i32,
    pub title: Option<String>,
    pub catalog: Option<String>,
    pub artist: Option<String>,
    pub created_at: crate::DateTime,
    pub updated_at: crate::DateTime,
    pub tracks: Vec<Track>,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct Track {
    pub id: cynic::Id,
    pub index: i32,
    pub title: String,
    pub artist: String,
    pub artists: Option<crate::Json>,
    #[cynic(rename = "type")]
    pub type_: TrackType,
    pub created_at: crate::DateTime,
    pub updated_at: crate::DateTime,
}

#[derive(cynic::Enum, Clone, Copy, Debug)]
pub enum MetadataOrganizeLevel {
    Initial,
    Partial,
    Reviewed,
    Finished,
}

#[derive(cynic::Enum, Clone, Copy, Debug)]
pub enum TrackType {
    Normal,
    Instrumental,
    Absolute,
    Drama,
    Radio,
    Vocal,
    Unknown,
}
