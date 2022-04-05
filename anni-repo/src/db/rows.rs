use uuid::Uuid;
use crate::models::TrackType;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct AlbumRow {
    pub album_id: Uuid,
    pub title: String,
    pub edition: Option<String>,
    pub catalog: String,
    pub artist: String,
    pub release_date: String,
    pub album_type: String,
}

#[derive(Debug, Deserialize)]
pub struct DiscRow {
    pub album_id: Uuid,
    pub disc_id: u8,
    pub title: String,
    pub artist: String,
    pub catalog: String,
    pub disc_type: String,
}

#[derive(Debug, Deserialize)]
pub struct TrackRow {
    pub album_id: Uuid,
    pub disc_id: u8,
    pub track_id: u8,
    pub title: String,
    pub artist: String,
    pub track_type: String,
}

#[derive(Debug, Deserialize)]
pub struct TagRow {
    pub album_id: Uuid,
    pub disc_id: Option<u8>,
    pub track_id: Option<u8>,
    pub name: String,
    pub tag_type: TrackType,
}