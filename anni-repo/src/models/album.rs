use crate::prelude::*;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::ops::Deref;
use std::str::FromStr;
use toml_edit::easy as toml;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Album {
    #[serde(rename = "album")]
    info: AlbumInfo,
    discs: Vec<Disc>,
}

impl Album {
    pub fn new(info: AlbumInfo, discs: Vec<Disc>) -> Self {
        Album { info, discs }
    }
}

impl FromStr for Album {
    type Err = Error;

    fn from_str(toml_str: &str) -> Result<Self, Self::Err> {
        let album: Album = toml::from_str(toml_str).map_err(|e| Error::TomlParseError {
            target: "Album",
            input: toml_str.to_string(),
            err: e,
        })?;

        Ok(album)
    }
}

impl ToString for Album {
    fn to_string(&self) -> String {
        toml::to_string_pretty(&self).unwrap()
    }
}

impl Deref for Album {
    type Target = AlbumInfo;

    fn deref(&self) -> &Self::Target {
        &self.info
    }
}

impl Album {
    pub fn album_id(&self) -> Uuid {
        self.info.album_id
    }

    /// Only album title uses edition parameter.
    pub fn full_title(&self) -> Cow<str> {
        if let Some(edition) = &self.info.edition {
            Cow::Owned(format!("{}【{edition}】", self.info.title))
        } else {
            Cow::Borrowed(&self.info.title)
        }
    }

    pub fn title_raw(&self) -> &str {
        self.info.title.as_ref()
    }

    pub fn edition(&self) -> Option<&str> {
        self.info.edition.as_deref()
    }

    pub fn artist(&self) -> &str {
        self.info.artist.as_ref()
    }

    pub fn set_artist(&mut self, artist: String) {
        self.info.artist = artist;
    }

    pub fn release_date(&self) -> &AnniDate {
        &self.info.release_date
    }

    pub fn track_type(&self) -> TrackType {
        self.info.album_type.clone()
    }

    pub fn catalog(&self) -> &str {
        self.info.catalog.as_ref()
    }

    pub fn set_catalog(&mut self, catalog: String) {
        self.info.catalog = catalog;
    }

    pub fn tags(&self) -> Vec<&TagRef> {
        let mut tags = Vec::new();
        tags.append(&mut self.info.tags.iter().collect::<Vec<_>>());
        for disc in self.discs.iter() {
            if !disc.tags.is_empty() {
                tags.append(&mut disc.tags.iter().collect::<Vec<_>>());
            }
            for track in disc.tracks.iter() {
                if !track.tags.is_empty() {
                    tags.append(&mut track.tags.iter().collect::<Vec<_>>());
                }
            }
        }
        tags.into_iter()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect()
    }

    pub fn album_tags(&self) -> &[TagRef] {
        &self.info.tags
    }

    pub fn discs_len(&self) -> usize {
        self.discs.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = DiscRef> {
        self.discs.iter().map(move |disc| DiscRef {
            album: &self.info,
            disc,
        })
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = DiscRefMut> {
        let album = &self.info;
        self.discs.iter_mut().map(move |disc| DiscRefMut {
            album,
            disc: &disc.info,
            tracks: &mut disc.tracks,
        })
    }

    // // TODO: tests
    // pub fn fmt(&mut self, inherit: bool) {
    //     let mut owned_artist = None;
    //     for disc in self.discs.iter_mut() {
    //         if disc.artist.is_owned() {
    //             if let Some(owned_artist) = owned_artist {
    //                 if owned_artist != disc.artist.as_ref() {
    //                     return;
    //                 }
    //             } else {
    //                 owned_artist = Some(disc.artist.as_ref())
    //             }
    //         } else {
    //             return;
    //         }
    //     }
    //
    //     // all owned artists are the same, and self.artist is inherited
    //     if &self.info.artist == "UnknownArtist" {
    //         self.info.artist = owned_artist.unwrap().to_string().into();
    //         for disc in self.discs.iter_mut() {
    //             disc.artist = None.into();
    //             if inherit {
    //                 disc.artist.inherit_from_owned(&self.info.artist);
    //             }
    //         }
    //     }
    // }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct AlbumInfo {
    /// Album ID(uuid)
    pub album_id: Uuid,
    /// Album title
    pub title: String,
    /// Album edition
    ///
    /// If this field is not None and is not empty, the full title of Album should be {title}【{edition}】
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(with = "serde_with::rust::string_empty_as_none")]
    pub edition: Option<String>,
    /// Album artist
    pub artist: String,
    /// Album artists
    #[serde(default)]
    #[serde(skip_serializing_if = "is_artists_empty")]
    pub artists: Option<HashMap<String, String>>,
    /// Album release date
    #[serde(rename = "date")]
    pub release_date: AnniDate,
    /// Album track type
    #[serde(rename = "type")]
    pub album_type: TrackType,
    /// Album catalog
    pub catalog: String,
    /// Album tags
    #[serde(default)]
    pub tags: Vec<TagRef>,
}

impl Default for AlbumInfo {
    fn default() -> Self {
        Self {
            album_id: Uuid::new_v4(),
            title: "UnknownTitle".to_string().into(),
            edition: None,
            artist: "UnknownArtist".to_string().into(),
            artists: HashMap::new().into(),
            release_date: AnniDate::new(2021, 1, 1),
            album_type: TrackType::Normal,
            catalog: "@TEMP".to_string(),
            tags: Default::default(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Disc {
    #[serde(flatten)]
    info: DiscInfo,
    tracks: Vec<TrackInfo>,
}

impl Disc {
    pub fn new(info: DiscInfo, tracks: Vec<TrackInfo>) -> Self {
        Self { info, tracks }
    }
}

impl Deref for Disc {
    type Target = DiscInfo;

    fn deref(&self) -> &Self::Target {
        &self.info
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct DiscInfo {
    /// Disc title
    pub title: Option<String>,
    /// Disc artist
    pub artist: Option<String>,
    /// Disc artists
    #[serde(skip_serializing_if = "is_artists_empty")]
    pub artists: Option<HashMap<String, String>>,
    /// Disc catalog
    pub catalog: String,
    /// Disc type
    #[serde(rename = "type")]
    pub disc_type: Option<TrackType>,
    /// Disc tags
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<TagRef>,
}

impl DiscInfo {
    pub fn new(
        catalog: String,
        title: Option<String>,
        artist: Option<String>,
        disc_type: Option<TrackType>,
        tags: Vec<TagRef>,
    ) -> Self {
        DiscInfo {
            title,
            artist,
            artists: Default::default(),
            catalog,
            tags,
            disc_type,
        }
    }

    // pub fn fmt(&mut self, inherit: bool) {
    //     let mut owned_artist = None;
    //     for track in self.tracks.iter_mut() {
    //         if track.artist.is_owned() {
    //             if let Some(owned_artist) = owned_artist {
    //                 if owned_artist != track.artist.as_ref() {
    //                     return;
    //                 }
    //             } else {
    //                 owned_artist = Some(track.artist.as_ref())
    //             }
    //         } else {
    //             return;
    //         }
    //     }
    //
    //     // all owned artists are the same, and self.artist is inherited
    //     if self.artist.is_inherited() {
    //         self.artist = owned_artist.unwrap().to_string().into();
    //         for track in self.tracks.iter_mut() {
    //             track.artist = InheritableValue::Inherited(None);
    //             if inherit {
    //                 track.artist.inherit_from(&self.artist);
    //             }
    //         }
    //     }
    // }
}

#[derive(Clone)]
pub struct DiscRef<'album> {
    pub(crate) album: &'album AlbumInfo,
    pub(crate) disc: &'album Disc,
}

impl<'album> DiscRef<'album> {
    pub fn title(&self) -> &str {
        self.disc
            .title
            .as_deref()
            .unwrap_or_else(|| self.album.title.as_str())
    }

    pub fn artist(&self) -> &str {
        self.disc
            .artist
            .as_deref()
            .unwrap_or_else(|| self.album.artist.as_str())
    }

    pub fn catalog(&self) -> &str {
        self.disc.catalog.as_ref()
    }

    pub fn track_type(&self) -> &TrackType {
        self.disc
            .disc_type
            .as_ref()
            .unwrap_or_else(|| &self.album.album_type)
    }

    pub fn tags(&self) -> &[TagRef] {
        self.disc.tags.as_ref()
    }

    pub fn tracks_len(&self) -> usize {
        self.disc.tracks.len()
    }

    pub fn iter<'disc>(&'disc self) -> impl Iterator<Item = TrackRef<'album, 'disc>> {
        self.disc.tracks.iter().map(move |track| TrackRef {
            album: self.album,
            disc: self.disc,
            track,
        })
    }
}

pub struct DiscRefMut<'album> {
    pub(crate) album: &'album AlbumInfo,
    pub(crate) disc: &'album DiscInfo,
    pub(crate) tracks: &'album mut Vec<TrackInfo>,
}

impl<'album> DiscRefMut<'album> {
    pub fn iter_mut(&mut self) -> impl Iterator<Item = TrackRefMut> {
        let album = self.album;
        let disc = self.disc;
        self.tracks
            .iter_mut()
            .map(move |track| TrackRefMut { album, disc, track })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct TrackInfo {
    /// Track title
    pub title: String,
    /// Track artist
    pub artist: Option<String>,
    /// Track artists
    #[serde(skip_serializing_if = "is_artists_empty")]
    pub artists: Option<HashMap<String, String>>,
    /// Track type
    #[serde(rename = "type")]
    pub track_type: Option<TrackType>,
    /// Track tags
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<TagRef>,
}

impl TrackInfo {
    pub fn new(
        title: String,
        artist: Option<String>,
        track_type: Option<TrackType>,
        tags: Vec<TagRef>,
    ) -> Self {
        TrackInfo {
            title,
            artist,
            artists: Default::default(),
            track_type,
            tags,
        }
    }

    pub fn empty() -> Self {
        TrackInfo::new(String::new(), None, None, Default::default())
    }

    pub fn set_title(&mut self, title: String) {
        self.title = title;
    }
}

#[derive(Clone)]
pub struct TrackRef<'album, 'disc> {
    pub(crate) album: &'album AlbumInfo,
    pub(crate) disc: &'disc DiscInfo,
    pub(crate) track: &'disc TrackInfo,
}

impl<'a, 'd> TrackRef<'a, 'd> {
    pub fn title(&self) -> &str {
        self.track.title.as_ref()
    }

    pub fn artist(&self) -> &str {
        self.track.artist.as_deref().unwrap_or_else(|| {
            self.disc
                .artist
                .as_deref()
                .unwrap_or_else(|| self.album.artist.as_str())
        })
    }

    pub fn artists(&self) -> Option<&HashMap<String, String>> {
        self.track.artists.as_ref().or_else(|| {
            self.disc
                .artists
                .as_ref()
                .or_else(|| self.album.artists.as_ref())
        })
    }

    pub fn track_type(&self) -> &TrackType {
        self.track.track_type.as_ref().unwrap_or_else(|| {
            &self
                .disc
                .disc_type
                .as_ref()
                .unwrap_or_else(|| &self.album.album_type)
        })
    }

    pub fn tags(&self) -> &[TagRef] {
        self.track.tags.as_ref()
    }
}

pub struct TrackRefMut<'album, 'disc> {
    pub(crate) album: &'album AlbumInfo,
    pub(crate) disc: &'disc DiscInfo,
    pub(crate) track: &'disc mut TrackInfo,
}

impl TrackRefMut<'_, '_> {
    pub fn set_artist(&mut self, artist: Option<String>) {
        if let Some(artist) = artist {
            let artist_str = artist.as_str();
            let current_artist_str = self.track.artist.as_deref().unwrap_or_else(|| {
                self.disc
                    .artist
                    .as_deref()
                    .unwrap_or_else(|| self.album.artist.as_str())
            });

            if artist_str == current_artist_str {
                self.track.artist = None;
            } else {
                self.track.artist = Some(artist);
            }
        } else {
            self.track.artist = None;
        }
    }

    pub fn set_track_type(&mut self, track_type: Option<TrackType>) {
        self.track.track_type = track_type;
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrackType {
    Normal,
    Instrumental,
    Absolute,
    Drama,
    Radio,
    Vocal,
}

impl AsRef<str> for TrackType {
    fn as_ref(&self) -> &str {
        match &self {
            TrackType::Normal => "normal",
            TrackType::Instrumental => "instrumental",
            TrackType::Absolute => "absolute",
            TrackType::Drama => "drama",
            TrackType::Radio => "radio",
            TrackType::Vocal => "vocal",
        }
    }
}

impl TrackType {
    pub fn guess(title: &str) -> Option<TrackType> {
        let title_lowercase = title.to_lowercase();
        if title_lowercase.contains("off vocal")
            || title_lowercase.contains("instrumental")
            || title_lowercase.contains("カラオケ")
            || title_lowercase.contains("offvocal")
        {
            Some(TrackType::Instrumental)
        } else if title_lowercase.contains("drama") || title_lowercase.contains("ドラマ") {
            Some(TrackType::Drama)
        } else if title_lowercase.contains("radio") || title_lowercase.contains("ラジオ") {
            Some(TrackType::Radio)
        } else {
            None
        }
    }
}

fn is_artists_empty(artists: &Option<HashMap<String, String>>) -> bool {
    match artists {
        Some(artists) => artists.is_empty(),
        None => true,
    }
}

#[cfg(feature = "flac")]
impl From<anni_flac::FlacHeader> for TrackInfo {
    fn from(stream: anni_flac::FlacHeader) -> Self {
        use crate::library::file_stem;
        use regex::Regex;

        match stream.comments() {
            Some(comment) => {
                let map = comment.to_map();
                let title = map
                    .get("TITLE")
                    .map(|v| v.value().to_owned())
                    .or_else(|| {
                        // use filename as default track name
                        let reg = Regex::new(r#"^\d{2,3}(?:\s?[.-]\s?|\s)(.+)$"#).unwrap();
                        let input = file_stem(&stream.path).ok()?;
                        let filename = reg
                            .captures(&input)
                            .and_then(|c| c.get(1))
                            .map(|r| r.as_str().to_string())
                            .unwrap_or_else(|| input);
                        Some(filename)
                    })
                    .unwrap_or_default();
                // auto audio type for instrumental, drama and radio
                let track_type = TrackType::guess(&title);
                TrackInfo::new(
                    title,
                    map.get("ARTIST").map(|v| v.value().to_string()),
                    track_type,
                    Default::default(),
                )
            }
            None => TrackInfo::empty(),
        }
    }
}
