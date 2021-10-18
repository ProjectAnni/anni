use serde::{Serialize, Deserialize, Deserializer, Serializer};
use std::str::FromStr;
use std::path::Path;
use anni_common::traits::FromFile;
use anni_derive::FromFile;
use anni_common::inherit::InheritableValue;
use crate::date::AnniDate;

#[derive(Serialize, Deserialize, FromFile)]
pub struct Album {
    #[serde(rename = "album")]
    info: AlbumInfo,
    discs: Vec<Disc>,
}

impl Album {
    pub fn new(title: String, artist: String, release_date: AnniDate, catalog: String) -> Self {
        Album {
            info: AlbumInfo {
                title: InheritableValue::own(title),
                artist: InheritableValue::own(artist),
                release_date,
                album_type: TrackType::Normal, // TODO: custom album type
                catalog,
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
    pub fn title(&self) -> &str {
        self.info.title.as_ref()
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
    title: InheritableValue<String>,
    artist: InheritableValue<String>,
    #[serde(rename = "date")]
    release_date: AnniDate,
    #[serde(rename = "type")]
    album_type: TrackType,
    catalog: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Disc {
    catalog: String,
    title: InheritableValue<String>,
    artist: InheritableValue<String>,
    #[serde(rename = "type")]
    disc_type: InheritableValue<TrackType>,
    tracks: Vec<Track>,
}

impl Disc {
    pub fn new<I, T>(catalog: String, title: I, artist: I, disc_type: T) -> Self
        where
            I: Into<InheritableValue<String>>, T: Into<InheritableValue<TrackType>> {
        Disc {
            catalog,
            title: title.into(),
            artist: artist.into(),
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
        let mut album = Album::new(title, self.artist.as_ref().to_string(), release_date, self.catalog.to_string());
        self.title.reset();
        self.artist.reset();
        self.disc_type.reset();
        album.add_disc(self);
        album
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Track {
    title: String,
    artist: InheritableValue<String>,
    #[serde(rename = "type")]
    track_type: InheritableValue<TrackType>,
}

impl Track {
    pub fn new<I, T>(title: String, artist: I, track_type: T) -> Self
        where
            I: Into<InheritableValue<String>>, T: Into<InheritableValue<TrackType>> {
        Track {
            title,
            artist: artist.into(),
            track_type: track_type.into(),
        }
    }

    pub fn empty() -> Self {
        Track::new(String::new(), None, None)
    }

    pub fn title(&self) -> &str {
        self.title.as_ref()
    }

    pub fn artist(&self) -> &str {
        self.artist.as_ref()
    }

    pub fn track_type(&self) -> TrackType {
        self.track_type.get_raw()
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
