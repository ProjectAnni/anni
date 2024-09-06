use crate::annim::query::album::{AlbumFragment, TrackTypeInput};
use crate::annim::{schema, Json, Uuid};

#[derive(cynic::QueryVariables, Debug)]
pub struct AddAlbumVariables<'a> {
    pub album: AddAlbumInput<'a>,
    pub commit: Option<bool>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "MetadataMutation", variables = "AddAlbumVariables")]
pub struct AddAlbumMutation {
    #[arguments(input: $album, commit: $commit)]
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

impl<'album, 'disc> From<crate::model::TrackRef<'album, 'disc>> for CreateAlbumTrackInput<'album>
where
    'disc: 'album,
{
    fn from(track: crate::model::TrackRef<'album, 'disc>) -> Self {
        Self {
            title: track.title(),
            artist: track.artist(),
            type_: track.track_type().into(),
        }
    }
}

impl<'a> From<&'a crate::model::Track> for CreateAlbumTrackInput<'a> {
    fn from(track: &'a crate::model::Track) -> Self {
        Self {
            title: &track.title,
            artist: track
                .artist
                .as_deref()
                .unwrap_or(crate::model::UNKNOWN_ARTIST),
            type_: track
                .track_type
                .as_ref()
                .unwrap_or_else(|| &crate::model::TrackType::Normal)
                .into(),
        }
    }
}

impl<'a> From<&'a crate::model::Disc> for CreateAlbumDiscInput<'a> {
    fn from(value: &'a crate::model::Disc) -> Self {
        Self {
            title: value.title.as_deref(),
            catalog: Some(value.catalog.as_str()),
            artist: value.artist.as_deref(),
            tracks: value.tracks.iter().map(Into::into).collect(),
        }
    }
}
