use serde::{Serialize, Deserialize, Serializer, Deserializer};
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
    #[serde(rename(serialize = "type"))]
    pub album_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DiscRow {
    pub album_id: UuidRow,
    pub disc_id: u8,
    pub title: String,
    pub artist: String,
    pub catalog: String,
    #[serde(rename(serialize = "type"))]
    pub disc_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TrackRow {
    pub album_id: UuidRow,
    pub disc_id: u8,
    pub track_id: u8,
    pub title: String,
    pub artist: String,
    #[serde(rename(serialize = "type"))]
    pub track_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TagRow {
    pub album_id: UuidRow,
    pub disc_id: Option<u8>,
    pub track_id: Option<u8>,
    pub name: String,
    #[serde(rename(serialize = "type"))]
    pub tag_type: String,
}

#[cfg(target_arch = "wasm32")]
pub mod wasm {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen(typescript_custom_section)]
    const ALBUM_ROW: &'static str = r#"
interface AlbumRow {
    album_id: string;
    title: string;
    edition?: string;
    catalog: string;
    artist: string;
    release_date: string;
    type: string;
}
"#;
    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(typescript_type = "AlbumRow")]
        pub type IAlbumRow;
    }
}