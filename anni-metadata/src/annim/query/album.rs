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

impl AlbumFragment {
    pub fn release_date(&self) -> crate::model::AnniDate {
        crate::model::AnniDate::new(
            self.year as u16,
            self.month.unwrap_or(0) as u8,
            self.day.unwrap_or(0) as u8,
        )
    }
}

impl From<AlbumFragment> for crate::model::Album {
    fn from(album: AlbumFragment) -> Self {
        let release_date = album.release_date();
        crate::model::Album::new(
            crate::model::AlbumInfo {
                album_id: album.album_id,
                title: album.title,
                edition: album.edition,
                artist: album.artist,
                artists: None,
                release_date,
                album_type: crate::model::TrackType::Normal,
                catalog: album.catalog.unwrap_or_default(),
                tags: album.tags.into_iter().map(Into::into).collect(),
            },
            album.discs.into_iter().map(Into::into).collect(),
        )
    }
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

impl From<DiscFragment> for crate::model::Disc {
    fn from(disc: DiscFragment) -> Self {
        crate::model::Disc::new(
            crate::model::DiscInfo {
                title: disc.title,
                catalog: disc.catalog.unwrap_or_default(),
                artist: disc.artist,
                artists: None,
                disc_type: None,
                tags: disc.tags.into_iter().map(Into::into).collect(),
            },
            disc.tracks.into_iter().map(Into::into).collect(),
        )
    }
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

impl From<TrackFragment> for crate::model::Track {
    fn from(track: TrackFragment) -> Self {
        crate::model::Track {
            title: track.title,
            artist: Some(track.artist),
            artists: None, // TODO: parse artists
            track_type: Some(track.type_.into()),
            tags: track.tags.into_iter().map(Into::into).collect(),
        }
    }
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

impl From<TagBase> for crate::model::TagString {
    fn from(value: TagBase) -> Self {
        crate::model::TagString::new(value.name, value.type_.into())
    }
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

impl From<TagTypeInput> for crate::model::TagType {
    fn from(value: TagTypeInput) -> Self {
        match value {
            TagTypeInput::Artist => crate::model::TagType::Artist,
            TagTypeInput::Group => crate::model::TagType::Group,
            TagTypeInput::Animation => crate::model::TagType::Animation,
            TagTypeInput::Radio => crate::model::TagType::Radio,
            TagTypeInput::Series => crate::model::TagType::Series,
            TagTypeInput::Project => crate::model::TagType::Project,
            TagTypeInput::Game => crate::model::TagType::Game,
            TagTypeInput::Organization => crate::model::TagType::Organization,
            TagTypeInput::Category => crate::model::TagType::Category,
            TagTypeInput::Others => crate::model::TagType::Unknown,
        }
    }
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

impl From<TrackTypeInput> for crate::model::TrackType {
    fn from(value: TrackTypeInput) -> Self {
        match value {
            TrackTypeInput::Normal => crate::model::TrackType::Normal,
            TrackTypeInput::Instrumental => crate::model::TrackType::Instrumental,
            TrackTypeInput::Absolute => crate::model::TrackType::Absolute,
            TrackTypeInput::Drama => crate::model::TrackType::Drama,
            TrackTypeInput::Radio => crate::model::TrackType::Radio,
            TrackTypeInput::Vocal => crate::model::TrackType::Vocal,
            TrackTypeInput::Unknown => crate::model::TrackType::Normal,
        }
    }
}
