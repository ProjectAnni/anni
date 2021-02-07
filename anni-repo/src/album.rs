use toml::value::Datetime;
use serde::{Serialize, Deserialize, Deserializer, Serializer};
use std::str::FromStr;
use std::path::Path;

#[derive(Serialize, Deserialize)]
pub struct Album {
    #[serde(rename = "album")]
    info: AlbumInfo,
    discs: Vec<Disc>,
}

impl Album {
    pub fn new(title: &str, artist: &str, release_date: Datetime, catalog: &str) -> Self {
        Album {
            info: AlbumInfo {
                title: title.to_owned(),
                artist: artist.to_owned(),
                release_date,
                track_type: TrackType::Normal,
                catalog: catalog.to_owned(),
            },
            discs: Vec::new(),
        }
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> Self {
        Self::from_str(&*std::fs::read_to_string(path.as_ref()).unwrap()).unwrap()
    }
}

impl FromStr for Album {
    type Err = Box<dyn std::error::Error>;

    fn from_str(toml_str: &str) -> Result<Self, Self::Err> {
        let mut album: Album = toml::from_str(toml_str)?;
        for disc in &mut album.discs {
            for track in &mut disc.tracks {
                if let None = track.artist {
                    track.artist = Some(album.info.artist.to_owned());
                }

                if let None = track.track_type {
                    track.track_type = Some((&album.info.track_type).to_owned());
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

    pub fn release_date(&self) -> String {
        self.info.release_date.to_string()
    }

    pub fn track_type(&self) -> TrackType {
        self.info.track_type.clone()
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
    track_type: TrackType,
    catalog: String,
}

#[derive(Serialize, Deserialize)]
pub struct Disc {
    catalog: String,
    tracks: Vec<Track>,
}

impl Disc {
    pub fn new(catalog: &str) -> Self {
        Disc {
            catalog: catalog.to_owned(),
            tracks: vec![],
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

#[derive(Serialize, Deserialize)]
pub struct Track {
    title: String,
    artist: Option<String>,
    #[serde(rename = "type")]
    track_type: Option<TrackType>,
    // TODO: lyric
}

impl Track {
    pub fn new(title: &str, artist: Option<&str>, track_type: Option<TrackType>) -> Self {
        Track {
            title: title.to_owned(),
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
