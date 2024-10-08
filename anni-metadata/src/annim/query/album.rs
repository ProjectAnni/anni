use crate::annim::{schema, DateTime, Json, Uuid};

#[derive(cynic::QueryVariables, Debug)]
pub struct AlbumVariables {
    pub album_id: Uuid,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "MetadataQuery", variables = "AlbumVariables")]
pub struct AlbumQuery {
    #[arguments(albumId: $album_id)]
    pub album: Option<AlbumFragment>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Album")]
pub struct AlbumFragment {
    pub id: cynic::Id,
    pub album_id: Uuid,
    pub level: MetadataOrganizeLevel,
    pub title: String,
    pub edition: Option<String>,
    pub catalog: Option<String>,
    pub artist: String,
    pub year: i32,
    pub month: Option<i32>,
    pub day: Option<i32>,
    pub tags: Vec<TagBase>,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub extra: Option<Json>,
    pub discs: Vec<DiscFragment>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Disc")]
pub struct DiscFragment {
    pub id: cynic::Id,
    pub index: i32,
    pub title: Option<String>,
    pub catalog: Option<String>,
    pub artist: Option<String>,
    pub tags: Vec<TagBase>,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub tracks: Vec<TrackFragment>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Track")]
pub struct TrackFragment {
    pub id: cynic::Id,
    pub index: i32,
    pub title: String,
    pub artist: String,
    #[cynic(rename = "type")]
    pub type_: TrackTypeInput,
    pub artists: Option<Json>,
    pub tags: Vec<TagBase>,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Tag")]
pub struct TagBase {
    pub id: cynic::Id,
    pub name: String,
    #[cynic(rename = "type")]
    pub type_: TagTypeInput,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(cynic::Enum, Clone, Copy, Debug)]
pub enum MetadataOrganizeLevel {
    Initial,
    Partial,
    Reviewed,
    Finished,
}

#[derive(cynic::Enum, Clone, Copy, Debug)]
#[cynic(graphql_type = "TrackType")]
pub enum TrackTypeInput {
    Normal,
    Instrumental,
    Absolute,
    Drama,
    Radio,
    Vocal,
    Unknown,
}

#[derive(cynic::Enum, Clone, Copy, Debug)]
#[cynic(graphql_type = "TagType")]
pub enum TagTypeInput {
    Artist,
    Group,
    Animation,
    Radio,
    Series,
    Project,
    Game,
    Organization,
    Category,
    Others,
}

impl From<&crate::model::TrackType> for TrackTypeInput {
    fn from(value: &crate::model::TrackType) -> Self {
        match value {
            crate::model::TrackType::Normal => TrackTypeInput::Normal,
            crate::model::TrackType::Instrumental => TrackTypeInput::Instrumental,
            crate::model::TrackType::Absolute => TrackTypeInput::Absolute,
            crate::model::TrackType::Drama => TrackTypeInput::Drama,
            crate::model::TrackType::Radio => TrackTypeInput::Radio,
            crate::model::TrackType::Vocal => TrackTypeInput::Vocal,
        }
    }
}
