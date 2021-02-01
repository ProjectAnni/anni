use toml::value::Datetime;
use serde::{Deserialize, Deserializer, de};
use std::marker::PhantomData;
use serde::export::fmt;
use std::str::FromStr;

#[derive(Debug, Deserialize, PartialEq)]
pub struct Album {
    #[serde(rename = "album")]
    album_info: AlbumInfo,
    catalog: Catalog,
    discs: Vec<Disc>,
}

impl FromStr for Album {
    type Err = toml::de::Error;

    fn from_str(toml_str: &str) -> Result<Self, Self::Err> {
        toml::from_str(toml_str)
    }
}

#[derive(Debug, Deserialize, PartialEq)]
struct AlbumInfo {
    title: String,
    #[serde(default)]
    #[serde(deserialize_with = "string_or_seq_string")]
    artist: Option<Vec<String>>,
    #[serde(rename = "date")]
    release_date: Datetime,
    #[serde(rename = "type")]
    track_type: Option<TrackType>,
}

#[derive(Debug, Deserialize, PartialEq)]
struct Catalog {
    anime: Option<String>,
    #[serde(rename = "fanclub-limited")]
    fanclub_limited: Option<String>,
    #[serde(rename = "full-limited")]
    full_limited: Option<String>,
    limited: Option<String>,
    regular: Option<String>,
    all: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct Disc {
    catalog: Option<String>,
    tracks: Vec<Track>,
}

#[derive(Debug, Deserialize, PartialEq)]
struct Track {
    title: String,
    #[serde(default)]
    #[serde(deserialize_with = "string_or_seq_string")]
    artist: Option<Vec<String>>,
    #[serde(rename = "type")]
    track_type: Option<TrackType>,
}

#[derive(Debug, PartialEq)]
enum TrackType {
    Normal,
    OffVocal,
    Instrumental,
    Drama,
    Radio,
    Other(String),
}

impl<'de> Deserialize<'de> for TrackType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'de>
    {
        let s = String::deserialize(deserializer)?;
        Ok(match s.as_str() {
            "normal" => TrackType::Normal,
            "off-vocal" => TrackType::OffVocal,
            "instrumental" => TrackType::Instrumental,
            "drama" => TrackType::Drama,
            "radio" => TrackType::Radio,
            _ => TrackType::Other(s),
        })
    }
}

fn string_or_seq_string<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
    where D: Deserializer<'de>
{
    struct StringOrVec(PhantomData<Option<Vec<String>>>);

    impl<'de> de::Visitor<'de> for StringOrVec {
        type Value = Option<Vec<String>>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("string or list of strings")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where E: de::Error
        {
            Ok(Some(vec![value.to_owned()]))
        }

        fn visit_seq<S>(self, visitor: S) -> Result<Self::Value, S::Error>
            where S: de::SeqAccess<'de>
        {
            Deserialize::deserialize(de::value::SeqAccessDeserializer::new(visitor))
        }
    }

    deserializer.deserialize_any(StringOrVec(PhantomData))
}
