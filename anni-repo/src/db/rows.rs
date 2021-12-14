use sqlx::FromRow;
use uuid::Uuid;

#[derive(FromRow, Debug)]
pub struct AlbumRow {
    album_id: Uuid,
    title: String,
    edition: Option<String>,
    catalog: String,
    artist: String,
    release_date: String,
    album_type: String,
}

#[derive(FromRow, Debug)]
pub struct DiscRow {
    album_id: Uuid,
    disc_id: u8,
    title: String,
    artist: String,
    catalog: String,
    disc_type: String,
}

#[derive(FromRow, Debug)]
pub struct TrackRow {
    album_id: Uuid,
    disc_id: u8,
    track_id: u8,
    title: String,
    artist: String,
    track_type: String,
}

#[derive(FromRow, Debug)]
pub struct TagRow {
    album_id: Uuid,
    disc_id: Option<u8>,
    track_id: Option<u8>,
    name: String,
    edition: Option<String>,
}