use toml::value::Datetime;
use serde::{Serialize, Deserialize, Deserializer, Serializer};
use std::str::FromStr;

#[derive(Serialize, Deserialize)]
pub struct Album {
    #[serde(rename = "album")]
    info: AlbumInfo,
    discs: Vec<Disc>,
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
        toml::to_string_pretty(&self).unwrap()
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
    pub fn catalog(&self) -> &str {
        self.catalog.as_ref()
    }

    pub fn tracks(&self) -> &Vec<Track> {
        self.tracks.as_ref()
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

#[derive(Clone, PartialEq, Debug)]
pub enum TrackType {
    Normal,
    Instrumental,
    Absolute,
    Drama,
    Radio,
    Other(String),
}

impl ToString for TrackType {
    fn to_string(&self) -> String {
        match self {
            TrackType::Normal => "normal".to_owned(),
            TrackType::Instrumental => "instrumental".to_owned(),
            TrackType::Absolute => "absolute".to_owned(),
            TrackType::Drama => "drama".to_owned(),
            TrackType::Radio => "radio".to_owned(),
            TrackType::Other(s) => s.to_owned(),
        }
    }
}

impl Serialize for TrackType {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error> where
        S: Serializer {
        serializer.serialize_str(&self.to_string().as_ref())
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
