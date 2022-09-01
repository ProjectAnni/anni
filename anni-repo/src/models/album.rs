use crate::prelude::*;
use anni_common::inherit::{default_some, InheritableValue};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Album {
    #[serde(rename = "album")]
    info: AlbumInfo,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    discs: Vec<Disc>,
}

impl Album {
    pub fn new(
        title: String,
        edition: Option<String>,
        artist: String,
        release_date: AnniDate,
        catalog: String,
        tags: Vec<TagRef>,
    ) -> Self {
        Album {
            info: AlbumInfo {
                album_id: Uuid::new_v4(),
                title: InheritableValue::own(title),
                edition,
                catalog,
                artist: InheritableValue::own(artist),
                // TODO: specify artists
                artists: InheritableValue::own(Default::default()),
                release_date,
                tags,
                album_type: TrackType::Normal, // TODO: custom album type
            },
            discs: Vec::new(),
        }
    }
}

impl FromStr for Album {
    type Err = Error;

    fn from_str(toml_str: &str) -> Result<Self, Self::Err> {
        let mut album: Album = toml::from_str(toml_str).map_err(|e| Error::TomlParseError {
            target: "Album",
            input: toml_str.to_string(),
            err: e,
        })?;

        album.inherit();
        Ok(album)
    }
}

impl ToString for Album {
    fn to_string(&self) -> String {
        toml::to_string(&self).unwrap()
    }
}

impl Album {
    pub fn album_id(&self) -> Uuid {
        self.info.album_id
    }

    /// Only album title uses edition parameter.
    pub fn title(&self) -> Cow<str> {
        if let Some(edition) = &self.info.edition {
            Cow::Owned(format!("{}【{}】", self.info.title.as_ref(), edition))
        } else {
            Cow::Borrowed(self.info.title.as_ref())
        }
    }

    pub fn title_raw(&self) -> &str {
        self.info.title.as_ref()
    }

    pub fn edition_raw(&self) -> Option<&str> {
        self.info.edition.as_deref()
    }

    pub fn artist(&self) -> &str {
        self.info.artist.as_ref()
    }

    pub fn set_artist<T: Into<InheritableValue<String>>>(&mut self, artist: T) {
        self.info.artist = artist.into();
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

    pub fn tags_raw(&self) -> &[TagRef] {
        &self.info.tags
    }

    pub fn discs(&self) -> &Vec<Disc> {
        &self.discs
    }

    pub fn discs_mut(&mut self) -> &mut Vec<Disc> {
        &mut self.discs
    }

    // TODO: tests
    pub fn fmt(&mut self, inherit: bool) {
        let mut owned_artist = None;
        for disc in self.discs.iter_mut() {
            if disc.artist.is_owned() {
                if let Some(owned_artist) = owned_artist {
                    if owned_artist != disc.artist.as_ref() {
                        return;
                    }
                } else {
                    owned_artist = Some(disc.artist.as_ref())
                }
            } else {
                return;
            }
        }

        // all owned artists are the same, and self.artist is inherited
        if self.info.artist.as_ref() == "UnknownArtist" {
            self.info.artist = InheritableValue::own(owned_artist.unwrap().to_string());
            for disc in self.discs.iter_mut() {
                disc.artist = InheritableValue::Inherited(None);
                if inherit {
                    disc.artist.inherit_from(&self.info.artist);
                }
            }
        }
    }

    pub fn inherit(&mut self) {
        for disc in self.discs.iter_mut() {
            disc.title.inherit_from(&self.info.title);
            disc.artist.inherit_from(&self.info.artist);
            disc.disc_type.inherit_from_owned(&self.info.album_type);
            disc.artists.inherit_from(&self.info.artists);
            disc.inherit();
        }
    }

    pub fn add_disc(&mut self, mut disc: Disc) {
        disc.title.inherit_from(&self.info.title);
        disc.artist.inherit_from(&self.info.artist);
        disc.disc_type.inherit_from_owned(&self.info.album_type);
        disc.artists.inherit_from(&self.info.artists);
        self.push_disc(disc);
    }

    pub fn push_disc(&mut self, disc: Disc) {
        self.discs.push(disc);
    }

    pub fn into_discs(self) -> Vec<Disc> {
        self.discs
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AlbumInfo {
    /// Album ID(uuid)
    album_id: Uuid,
    /// Album title
    title: InheritableValue<String>,
    /// Album edition
    ///
    /// If this field is not None and is not empty, the full title of Album should be {title}【{edition}】
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(with = "serde_with::rust::string_empty_as_none")]
    edition: Option<String>,
    /// Album artist
    artist: InheritableValue<String>,
    /// Album artists
    #[serde(default = "default_some")]
    #[serde(skip_serializing_if = "is_artists_empty")]
    artists: InheritableValue<HashMap<String, String>>,
    /// Album release date
    #[serde(rename = "date")]
    release_date: AnniDate,
    /// Album track type
    #[serde(rename = "type")]
    album_type: TrackType,
    /// Album catalog
    catalog: String,
    /// Album tags
    #[serde(default)]
    tags: Vec<TagRef>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Disc {
    /// Disc title
    title: InheritableValue<String>,
    /// Disc artist
    artist: InheritableValue<String>,
    /// Disc artists
    #[serde(skip_serializing_if = "is_artists_empty")]
    artists: InheritableValue<HashMap<String, String>>,
    /// Disc catalog
    catalog: String,
    /// Disc type
    #[serde(rename = "type")]
    disc_type: InheritableValue<TrackType>,
    /// Disc tags
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tags: Vec<TagRef>,
    /// Disc tracks
    tracks: Vec<Track>,
}

impl Disc {
    pub fn new<I, T>(catalog: String, title: I, artist: I, disc_type: T, tags: Vec<TagRef>) -> Self
    where
        I: Into<InheritableValue<String>>,
        T: Into<InheritableValue<TrackType>>,
    {
        Disc {
            title: title.into(),
            artist: artist.into(),
            artists: Default::default(),
            catalog,
            tags,
            disc_type: disc_type.into(),
            tracks: Vec::new(),
        }
    }

    pub fn title(&self) -> &str {
        self.title.as_ref()
    }

    pub fn artist(&self) -> &str {
        self.artist.as_ref()
    }

    pub fn catalog(&self) -> &str {
        self.catalog.as_ref()
    }

    pub fn track_type(&self) -> &TrackType {
        self.disc_type.as_ref()
    }

    pub fn tags(&self) -> &[TagRef] {
        self.tags.as_ref()
    }

    pub fn tracks(&self) -> &Vec<Track> {
        self.tracks.as_ref()
    }

    pub fn tracks_mut(&mut self) -> &mut Vec<Track> {
        &mut self.tracks
    }

    pub fn inherit(&mut self) {
        for track in self.tracks.iter_mut() {
            track.artist.inherit_from(&self.artist);
            track.track_type.inherit_from(&self.disc_type);
            track.artists.inherit_from(&self.artists);
        }
    }

    pub fn add_track(&mut self, mut track: Track) {
        track.artist.inherit_from(&self.artist);
        track.track_type.inherit_from(&self.disc_type);
        track.artists.inherit_from(&self.artists);
        self.push_track(track);
    }

    pub fn push_track(&mut self, track: Track) {
        self.tracks.push(track);
    }

    pub fn into_album(mut self, title: String, release_date: AnniDate) -> Album {
        let mut album = Album::new(
            title,
            None,
            self.artist.as_ref().to_string(),
            release_date,
            self.catalog.to_string(),
            Default::default(),
        );
        self.title.reset();
        self.artist.reset();
        self.disc_type.reset();
        album.add_disc(self);
        album
    }

    pub fn fmt(&mut self, inherit: bool) {
        let mut owned_artist = None;
        for track in self.tracks.iter_mut() {
            if track.artist.is_owned() {
                if let Some(owned_artist) = owned_artist {
                    if owned_artist != track.artist.as_ref() {
                        return;
                    }
                } else {
                    owned_artist = Some(track.artist.as_ref())
                }
            } else {
                return;
            }
        }

        // all owned artists are the same, and self.artist is inherited
        if self.artist.is_inherited() {
            self.artist = InheritableValue::own(owned_artist.unwrap().to_string());
            for track in self.tracks.iter_mut() {
                track.artist = InheritableValue::Inherited(None);
                if inherit {
                    track.artist.inherit_from(&self.artist);
                }
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Track {
    /// Track title
    title: String,
    /// Track artist
    artist: InheritableValue<String>,
    /// Track artists
    #[serde(skip_serializing_if = "is_artists_empty")]
    artists: InheritableValue<HashMap<String, String>>,
    /// Track type
    #[serde(rename = "type")]
    track_type: InheritableValue<TrackType>,
    /// Track tags
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tags: Vec<TagRef>,
}

impl Track {
    pub fn new<I, T>(title: String, artist: I, track_type: T, tags: Vec<TagRef>) -> Self
    where
        I: Into<InheritableValue<String>>,
        T: Into<InheritableValue<TrackType>>,
    {
        Track {
            title,
            artist: artist.into(),
            artists: Default::default(),
            track_type: track_type.into(),
            tags,
        }
    }

    pub fn empty() -> Self {
        Track::new(String::new(), None, None, Default::default())
    }

    pub fn title(&self) -> &str {
        self.title.as_ref()
    }

    pub fn set_title(&mut self, title: String) {
        self.title = title;
    }

    pub fn artist(&self) -> &str {
        self.artist.as_ref()
    }

    pub fn set_artist<T: Into<InheritableValue<String>>>(&mut self, artist: T) {
        self.artist = artist.into();
    }

    pub fn track_type(&self) -> TrackType {
        self.track_type.get_raw()
    }

    pub fn set_track_type(&mut self, track_type: TrackType) {
        if self.track_type.is_inherited() || self.track_type.as_ref() != &track_type {
            self.track_type = InheritableValue::own(track_type);
        }
    }

    pub fn tags(&self) -> &[TagRef] {
        self.tags.as_ref()
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

fn is_artists_empty(artists: &InheritableValue<HashMap<String, String>>) -> bool {
    match artists {
        InheritableValue::Owned(artists) => artists.is_empty(),
        InheritableValue::Inherited(_) => true,
    }
}
