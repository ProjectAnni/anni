use std::{collections::HashMap, str::FromStr};

use crate::error::Error;
use anni_metadata::{
    annim::query::album::AlbumFragment,
    model::{Album, AlbumInfo, AnniDate, Disc, TagString, TrackType},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use anni_metadata::utils::is_artists_empty;

#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct JsonAlbum {
    #[serde(flatten)]
    info: JsonAlbumInfo,
    pub(crate) discs: Vec<Disc>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct JsonAlbumInfo {
    /// Album ID(uuid)
    pub album_id: Uuid,
    /// Album title
    pub title: String,
    /// Album edition
    ///
    /// If this field is not None and is not empty, the full title of Album should be {title}【{edition}】
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "anni_common::decode::non_empty_str")]
    pub edition: Option<String>,
    /// Album artist
    pub artist: String,
    /// Album artists
    #[serde(default)]
    #[serde(skip_serializing_if = "is_artists_empty")]
    pub artists: Option<HashMap<String, String>>,
    /// Album release date
    #[serde(rename = "date")]
    pub release_date: String,
    /// Album track type
    #[serde(rename = "type")]
    pub album_type: TrackType,
    /// Album catalog
    pub catalog: String,
    /// Album tags
    #[serde(default)]
    pub tags: Vec<TagString>,
}

impl FromStr for JsonAlbum {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s)
    }
}

impl ToString for JsonAlbum {
    fn to_string(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
}

impl From<AlbumFragment> for JsonAlbum {
    fn from(album: AlbumFragment) -> JsonAlbum {
        let album: Album = album.into();
        album.into()
    }
}

impl From<Album> for JsonAlbum {
    fn from(album: Album) -> JsonAlbum {
        let AlbumInfo {
            album_id,
            title,
            edition,
            artist,
            artists,
            release_date,
            album_type,
            catalog,
            tags,
        } = album.info;
        JsonAlbum {
            info: JsonAlbumInfo {
                album_id,
                title,
                edition,
                artist,
                artists,
                release_date: release_date.to_string(),
                album_type,
                catalog,
                tags,
            },
            discs: album.discs,
        }
    }
}

impl TryFrom<JsonAlbum> for Album {
    type Error = Error;

    fn try_from(album: JsonAlbum) -> Result<Self, Self::Error> {
        let JsonAlbumInfo {
            album_id,
            title,
            edition,
            artist,
            artists,
            release_date,
            album_type,
            catalog,
            tags,
        } = album.info;
        Ok(Album {
            info: AlbumInfo {
                album_id,
                title,
                edition,
                artist,
                artists,
                release_date: AnniDate::from_str(&release_date)?,
                album_type,
                catalog,
                tags,
            },
            discs: album.discs,
        })
    }
}

mod test {
    #[test]
    fn test_json_album_serialize_deserialize() {
        use super::JsonAlbum;
        use anni_metadata::model::Album;
        use std::str::FromStr;

        let text = include_str!("../../tests/fixtures/test-album.toml");

        let album = Album::from_str(text).unwrap();
        let album = JsonAlbum::from(album);
        let album = Album::try_from(album).unwrap();
        let album_serialized_text = toml::to_string_pretty(&album).unwrap();
        assert_eq!(text, album_serialized_text);
    }
}
