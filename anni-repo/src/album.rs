use serde::{Serialize, Deserialize, Deserializer, Serializer};
use std::str::FromStr;
use std::path::Path;
use crate::Datetime;
use anni_common::traits::FromFile;
use anni_derive::FromFile;

#[derive(Serialize, Deserialize, FromFile)]
pub struct Album {
    #[serde(rename = "album")]
    info: AlbumInfo,
    discs: Vec<Disc>,
}

impl Album {
    pub fn new(title: String, artist: String, release_date: Datetime, catalog: String) -> Self {
        Album {
            info: AlbumInfo {
                title,
                artist,
                release_date,
                album_type: TrackType::Normal,
                catalog,
            },
            discs: Vec::new(),
        }
    }

    pub fn format(&mut self) {
        let mut album_artist = String::new();
        for disc in self.discs.iter_mut() {
            for track in disc.tracks.iter_mut() {
                match &track.artist {
                    Some(artist) => {
                        if album_artist.is_empty() {
                            // the first track
                            album_artist = artist.clone()
                        } else if &album_artist != artist {
                            // not all artists are the same, exit
                            return;
                        }
                    }
                    None => {}
                }
            }
        }

        // all artists are the same, remove them in discs and tracks
        self.info.artist = album_artist;
        for disc in self.discs.iter_mut() {
            disc.artist = None;
            for track in disc.tracks.iter_mut() {
                track.artist = None;
            }
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
        for disc in &mut album.discs {
            if let None = disc.title {
                disc.title = Some(album.info.title.to_owned());
            }
            if let None = disc.artist {
                disc.artist = Some(album.info.artist.to_owned());
            }
            if let None = disc.disc_type {
                disc.disc_type = Some(album.info.album_type.to_owned());
            }
            for track in &mut disc.tracks {
                if let None = track.artist {
                    track.artist = Some(disc.artist.as_ref().unwrap().to_owned());
                }

                if let None = track.track_type {
                    track.track_type = Some((disc.disc_type.as_ref().unwrap()).to_owned());
                }
            }
        }
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

    pub fn release_date(&self) -> &Datetime {
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

    pub fn add_disc(&mut self, disc: Disc) {
        self.discs.push(disc);
    }
}

#[derive(Serialize, Deserialize)]
struct AlbumInfo {
    title: String,
    artist: String,
    #[serde(rename = "date")]
    release_date: Datetime,
    #[serde(rename = "type")]
    album_type: TrackType,
    catalog: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Disc {
    catalog: String,
    title: Option<String>,
    artist: Option<String>,
    #[serde(rename = "type")]
    disc_type: Option<TrackType>,
    tracks: Vec<Track>,
}

impl Disc {
    pub fn new(catalog: String, title: Option<String>, artist: Option<String>, disc_type: Option<TrackType>) -> Self {
        Disc {
            catalog,
            title,
            artist,
            disc_type,
            tracks: vec![],
        }
    }

    pub fn title(&self) -> &str {
        match &self.title {
            Some(title) => title,
            None => unreachable!(),
        }
    }

    pub fn artist(&self) -> &str {
        match &self.artist {
            Some(artist) => artist,
            None => unreachable!(),
        }
    }

    pub fn catalog(&self) -> &str {
        self.catalog.as_ref()
    }

    pub fn tracks(&self) -> &Vec<Track> {
        self.tracks.as_ref()
    }

    pub fn add_track(&mut self, track: Track) {
        self.tracks.push(track);
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Track {
    title: String,
    artist: Option<String>,
    #[serde(rename = "type")]
    track_type: Option<TrackType>,
}

impl Track {
    pub fn new(title: String, artist: Option<&str>, track_type: Option<TrackType>) -> Self {
        Track {
            title,
            artist: artist.map(|u| u.to_owned()),
            track_type,
        }
    }

    pub fn title(&self) -> &str {
        self.title.as_ref()
    }

    pub fn artist(&self) -> &str {
        match &self.artist {
            Some(a) => a.as_ref(),
            None => unreachable!(),
        }
    }

    pub fn track_type(&self) -> TrackType {
        match &self.track_type {
            Some(t) => t.clone(),
            None => unreachable!(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum TrackType {
    Normal,
    Instrumental,
    Absolute,
    Drama,
    Radio,
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
            _ => TrackType::Other(s),
        })
    }
}
