use serde::{Serialize, Deserialize, Deserializer, Serializer};
use std::str::FromStr;
use anni_derive::FromFile;
use anni_common::inherit::InheritableValue;
use crate::date::AnniDate;
use crate::tag::TagRef;
use std::borrow::Cow;

#[derive(Serialize, Deserialize, FromFile)]
pub struct Album {
    #[serde(rename = "album")]
    info: AlbumInfo,
    discs: Vec<Disc>,
}

impl Album {
    pub fn new(title: String, edition: Option<String>, artist: String, release_date: AnniDate, catalog: String, tags: Vec<TagRef>) -> Self {
        Album {
            info: AlbumInfo {
                title: InheritableValue::own(title),
                edition,
                catalog,
                artist: InheritableValue::own(artist),
                release_date,
                tags,
                album_type: TrackType::Normal, // TODO: custom album type
            },
            discs: Vec::new(),
        }
    }
}

impl FromStr for Album {
    type Err = crate::Error;

    fn from_str(toml_str: &str) -> Result<Self, Self::Err> {
        let mut album: Album = toml::from_str(toml_str)
            .map_err(|e| crate::Error::TomlParseError {
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
    /// Only album title uses edition parameter.
    pub fn title(&self) -> Cow<str> {
        if let Some(edition) = &self.info.edition {
            Cow::Owned(format!("{}【{}】", self.info.title.as_ref(), edition))
        } else {
            Cow::Borrowed(self.info.title.as_ref())
        }
    }

    pub fn artist(&self) -> &str {
        self.info.artist.as_ref()
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

    pub fn tags(&self) -> &[TagRef] {
        &self.info.tags
    }

    pub fn discs(&self) -> &Vec<Disc> {
        &self.discs
    }

    pub fn inherit(&mut self) {
        for disc in self.discs.iter_mut() {
            disc.title.inherit_from(&self.info.title);
            disc.artist.inherit_from(&self.info.artist);
            disc.disc_type.inherit_from_owned(&self.info.album_type);
            disc.inherit();
        }
    }

    pub fn add_disc(&mut self, mut disc: Disc) {
        disc.title.inherit_from(&self.info.title);
        disc.artist.inherit_from(&self.info.artist);
        disc.disc_type.inherit_from_owned(&self.info.album_type);
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
struct AlbumInfo {
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
pub struct Disc {
    /// Disc title
    title: InheritableValue<String>,
    /// Disc artist
    artist: InheritableValue<String>,
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
            I: Into<InheritableValue<String>>, T: Into<InheritableValue<TrackType>> {
        Disc {
            title: title.into(),
            artist: artist.into(),
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

    pub fn tracks(&self) -> &Vec<Track> {
        self.tracks.as_ref()
    }

    pub fn inherit(&mut self) {
        for track in self.tracks.iter_mut() {
            track.artist.inherit_from(&self.artist);
            track.track_type.inherit_from(&self.disc_type);
        }
    }

    pub fn add_track(&mut self, mut track: Track) {
        track.artist.inherit_from(&self.artist);
        track.track_type.inherit_from(&self.disc_type);
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
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Track {
    /// Track title
    title: String,
    /// Track artist
    artist: InheritableValue<String>,
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
            I: Into<InheritableValue<String>>, T: Into<InheritableValue<TrackType>> {
        Track {
            title,
            artist: artist.into(),
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

    pub fn track_type(&self) -> TrackType {
        self.track_type.get_raw()
    }

    pub fn set_track_type(&mut self, track_type: TrackType) {
        self.track_type = InheritableValue::own(track_type);
    }
}

#[derive(Clone, Debug)]
pub enum TrackType {
    Normal,
    Instrumental,
    Absolute,
    Drama,
    Radio,
    Vocal,
    Other(String),
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
            TrackType::Other(s) => s.as_ref(),
        }
    }
}

impl Serialize for TrackType {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error> where
        S: Serializer {
        serializer.serialize_str(self.as_ref())
    }
}

impl<'de> Deserialize<'de> for TrackType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'de>
    {
        let s = String::deserialize(deserializer)?;
        Ok(match s.as_str() {
            "normal" => TrackType::Normal,
            "instrumental" => TrackType::Instrumental,
            "absolute" => TrackType::Absolute,
            "drama" => TrackType::Drama,
            "radio" => TrackType::Radio,
            "vocal" => TrackType::Vocal,
            _ => TrackType::Other(s),
        })
    }
}
