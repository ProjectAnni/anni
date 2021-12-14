use sqlx::ConnectOptions;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode};
use uuid::Uuid;

mod rows;

pub struct RepoDatabase {
    conn: sqlx::SqliteConnection,
}

impl RepoDatabase {
    pub async fn create(path: &str) -> Result<Self, sqlx::Error> {
        let conn = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Off)
            .connect()
            .await?;
        let mut me = Self { conn };
        me.create_tables().await?;
        Ok(me)
    }

    pub async fn load(path: &str) -> Result<Self, sqlx::Error> {
        let conn = SqliteConnectOptions::new()
            .filename(path)
            .read_only(true)
            .connect()
            .await?;
        Ok(Self { conn })
    }

    async fn create_tables(&mut self) -> Result<(), sqlx::Error> {
        sqlx::query(r#"
CREATE TABLE IF NOT EXISTS "repo_album" (
  "album_id"       BLOB NOT NULL UNIQUE,
  "title"          TEXT NOT NULL,
  "edition"        TEXT,
  "catalog"        TEXT NOT NULL,
  "artist"         TEXT NOT NULL,
  "release_date"   TEXT NOT NULL,
  "album_type"     TEXT NOT NULL DEFAULT 'normal' CHECK("album_type" IN ('normal', 'instrumental', 'absolute', 'drama', 'radio', 'vocal'))
);
"#)
            .execute(&mut self.conn)
            .await?;

        sqlx::query(r#"
CREATE TABLE IF NOT EXISTS "repo_disc" (
  "album_id"    BLOB NOT NULL,
  "disc_id"     INTEGER NOT NULL,
  "title"       TEXT NOT NULL,
  "artist"      TEXT NOT NULL,
  "catalog"     TEXT NOT NULL,
  "disc_type"   TEXT NOT NULL DEFAULT 'normal' CHECK("disc_type" IN ('normal', 'instrumental', 'absolute', 'drama', 'radio', 'vocal')),
  UNIQUE("album_id","disc_id"),
  FOREIGN KEY("album_id") REFERENCES "repo_album"("album_id")
);
"#)
            .execute(&mut self.conn)
            .await?;

        sqlx::query(r#"
CREATE TABLE IF NOT EXISTS "repo_track" (
  "album_id"     BLOB NOT NULL,
  "disc_id"      INTEGER NOT NULL,
  "track_id"     TEXT NOT NULL,
  "title"        TEXT NOT NULL,
  "artist"       TEXT NOT NULL,
  "track_type"   TEXT NOT NULL DEFAULT 'normal' CHECK("track_type" IN ('normal', 'instrumental', 'absolute', 'drama', 'radio', 'vocal')),
  UNIQUE("album_id","disc_id","track_id"),
  FOREIGN KEY("album_id", "disc_id") REFERENCES "repo_disc"("album_id", "disc_id")
);
"#)
            .execute(&mut self.conn)
            .await?;

        sqlx::query(r#"
CREATE TABLE IF NOT EXISTS "repo_tag" (
  "album_id"    BLOB NOT NULL,
  "disc_id"     INTEGER,
  "track_id"    INTEGER,
  "name"        TEXT NOT NULL,
  "edition"     TEXT
);
"#)
            .execute(&mut self.conn)
            .await?;

        sqlx::query(r#"
CREATE TABLE IF NOT EXISTS "repo_info" (
  "key"    TEXT NOT NULL UNIQUE,
  "value"  TEXT
);
"#)
            .execute(&mut self.conn)
            .await?;

        Ok(())
    }

    pub async fn create_index(&mut self) -> Result<(), sqlx::Error> {
        sqlx::query(r#"
CREATE UNIQUE INDEX "repo_album_index" ON "repo_album" (
  "album_id"
);
"#)
            .execute(&mut self.conn)
            .await?;

        sqlx::query(r#"
CREATE UNIQUE INDEX IF NOT EXISTS "repo_disc_index" ON "repo_disc" (
  "album_id",
  "disc_id"
);
"#)
            .execute(&mut self.conn)
            .await?;

        sqlx::query(r#"
CREATE UNIQUE INDEX IF NOT EXISTS "repo_track_index" ON "repo_track" (
  "album_id",
  "disc_id",
  "track_id"
);
"#)
            .execute(&mut self.conn)
            .await?;

        sqlx::query(r#"
CREATE INDEX IF NOT EXISTS "repo_tag_index" ON "repo_tag" (
  "album_id",
  "disc_id",
  "track_id"
);
"#)
            .execute(&mut self.conn)
            .await?;
        Ok(())
    }

    pub async fn album(&mut self, album_id: Uuid) -> Result<Option<rows::AlbumRow>, sqlx::Error> {
        sqlx::query_as("SELECT * FROM repo_album WHERE album_id = ?")
            .bind(album_id)
            .fetch_optional(&mut self.conn)
            .await
    }

    pub async fn disc(&mut self, album_id: Uuid, disc_id: u8) -> Result<Option<rows::DiscRow>, sqlx::Error> {
        sqlx::query_as("SELECT * FROM repo_disc WHERE album_id = ? AND disc_id = ?")
            .bind(album_id)
            .bind(disc_id)
            .fetch_optional(&mut self.conn)
            .await
    }

    pub async fn track(&mut self, album_id: Uuid, disc_id: u8, track_id: u8) -> Result<Option<rows::TrackRow>, sqlx::Error> {
        sqlx::query_as("SELECT * FROM repo_track WHERE album_id = ? AND disc_id = ? AND track_id = ?")
            .bind(album_id)
            .bind(disc_id)
            .bind(track_id)
            .fetch_optional(&mut self.conn)
            .await
    }

    pub async fn add_album(&mut self, album: &crate::models::Album) -> Result<(), sqlx::Error> {
        let album_id = album.album_id();

        // add album info
        sqlx::query("INSERT INTO repo_album (album_id, title, edition, catalog, artist, release_date, album_type) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(&album_id)
            .bind(album.title_raw())
            .bind(album.edition_raw())
            .bind(album.catalog())
            .bind(album.artist())
            .bind(album.release_date().to_string())
            .bind(album.track_type().as_ref())
            .execute(&mut self.conn)
            .await?;

        // add album tags
        for tag in album.tags() {
            sqlx::query("INSERT INTO repo_tag (album_id, name, edition) VALUES (?, ?, ?)")
                .bind(&album_id)
                .bind(tag.name())
                .bind(tag.edition())
                .execute(&mut self.conn)
                .await?;
        }

        for (disc_id, disc) in album.discs().iter().enumerate() {
            let disc_id = disc_id + 1;

            // add disc info
            sqlx::query("INSERT INTO repo_disc (album_id, disc_id, title, artist, catalog, disc_type) VALUES (?, ?, ?, ?, ?, ?)")
                .bind(&album_id)
                .bind(disc_id as u8)
                .bind(disc.title())
                .bind(disc.artist())
                .bind(disc.catalog())
                .bind(disc.track_type().as_ref())
                .execute(&mut self.conn)
                .await?;

            // add disc tags
            for tag in disc.tags() {
                sqlx::query("INSERT INTO repo_tag (album_id, disc_id, name, edition) VALUES (?, ?, ?, ?)")
                    .bind(&album_id)
                    .bind(disc_id as u8)
                    .bind(tag.name())
                    .bind(tag.edition())
                    .execute(&mut self.conn)
                    .await?;
            }

            for (track_id, track) in disc.tracks().iter().enumerate() {
                let track_id = track_id + 1;

                // add track info
                sqlx::query("INSERT INTO repo_track (album_id, disc_id, track_id, title, artist, track_type) VALUES (?, ?, ?, ?, ?, ?)")
                    .bind(&album_id)
                    .bind(disc_id as u8)
                    .bind(track_id as u8)
                    .bind(track.title())
                    .bind(track.artist())
                    .bind(track.track_type().as_ref())
                    .execute(&mut self.conn)
                    .await?;

                // add track tags
                for tag in track.tags() {
                    sqlx::query("INSERT INTO repo_tag (album_id, disc_id, track_id, name, edition) VALUES (?, ?, ?, ?, ?)")
                        .bind(&album_id)
                        .bind(disc_id as u8)
                        .bind(track_id as u8)
                        .bind(tag.name())
                        .bind(tag.edition())
                        .execute(&mut self.conn)
                        .await?;
                }
            }
        }
        Ok(())
    }
}