use serde::{Serialize, Deserialize, Serializer, Deserializer};
use serde::de::DeserializeOwned;
use uuid::Uuid;

#[derive(Debug)]
pub struct UuidRow(pub Uuid);

impl<'de> Deserialize<'de> for UuidRow {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        let inner = Vec::deserialize(deserializer)?;
        Ok(UuidRow(Uuid::from_slice(&inner).unwrap()))
    }
}

impl Serialize for UuidRow {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        self.0.serialize(serializer)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AlbumRow {
    pub album_id: UuidRow,
    pub title: String,
    pub edition: Option<String>,
    pub catalog: String,
    pub artist: String,
    pub release_date: String,
    pub album_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DiscRow {
    pub album_id: UuidRow,
    pub disc_id: u8,
    pub title: String,
    pub artist: String,
    pub catalog: String,
    pub disc_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TrackRow {
    pub album_id: UuidRow,
    pub disc_id: u8,
    pub track_id: u8,
    pub title: String,
    pub artist: String,
    pub track_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TagRow {
    pub album_id: UuidRow,
    pub disc_id: Option<u8>,
    pub track_id: Option<u8>,
    pub name: String,
    pub tag_type: String,
}