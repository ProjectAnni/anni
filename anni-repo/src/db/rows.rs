use sqlx::FromRow;
use uuid::Uuid;

#[derive(FromRow, Debug)]
pub struct AlbumRow {
    pub album_id: Uuid,
    pub title: String,
    pub edition: Option<String>,
    pub catalog: String,
    pub artist: String,
    pub release_date: String,
    pub album_type: String,
}

#[derive(FromRow, Debug)]
pub struct DiscRow {
    pub album_id: Uuid,
    pub disc_id: u8,
    pub title: String,
    pub artist: String,
    pub catalog: String,
    pub disc_type: String,
}

#[derive(FromRow, Debug)]
pub struct TrackRow {
    pub album_id: Uuid,
    pub disc_id: u8,
    pub track_id: u8,
    pub title: String,
    pub artist: String,
    pub track_type: String,
}

#[derive(FromRow, Debug)]
pub struct TagRow {
    pub album_id: Uuid,
    pub disc_id: Option<u8>,
    pub track_id: Option<u8>,
    pub name: String,
    pub edition: Option<String>,
}