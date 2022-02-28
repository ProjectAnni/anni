use std::collections::HashMap;
use std::path::Path;
use sqlx::ConnectOptions;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous};
use uuid::Uuid;
use crate::prelude::RepoResult;

mod rows;

pub const DB_VERSION: &str = "1";

pub struct RepoDatabaseRead {
    uri: String,
    pool: sqlx::SqlitePool,
}

impl RepoDatabaseRead {
    pub async fn new(path: &str) -> RepoResult<Self> {
        Ok(Self {
            uri: path.to_string(),
            pool: sqlx::SqlitePool::connect(path).await?,
        })
    }

    pub async fn match_album(&self, catalog: &str, release_date: &crate::models::AnniDate, total_discs: u8, album_title: &str) -> RepoResult<Option<Uuid>> {
        let albums: Vec<(Uuid, String)> = sqlx::query_as("
SELECT album_id, title FROM repo_album
  WHERE catalog = ? AND release_date = ? AND disc_count = ?;
")
            .bind(catalog)
            .bind(release_date.to_string())
            .bind(total_discs)
            .fetch_all(&self.pool)
            .await?;
        if albums.is_empty() {
            Ok(None)
        } else if albums.len() == 1 {
            Ok(Some(albums[0].0))
        } else {
            let filtered: Vec<_> = albums
                .iter()
                .filter(|(_, title)| title == album_title)
                .collect();
            if filtered.is_empty() {
                Ok(None)
            } else if filtered.len() == 1 {
                Ok(Some(filtered[0].0))
            } else {
                log::warn!("Found multiple albums with the same catalog, release date, disc count and title: {:?}", filtered);
                log::warn!("Returning the first one");
                Ok(Some(filtered[0].0))
            }
        }
    }

    pub async fn album(&self, album_id: Uuid) -> RepoResult<Option<rows::AlbumRow>> {
        Ok(sqlx::query_as("SELECT * FROM repo_album WHERE album_id = ?")
            .bind(album_id)
            .fetch_optional(&self.pool)
            .await?)
    }

    pub async fn disc(&self, album_id: Uuid, disc_id: u8) -> RepoResult<Option<rows::DiscRow>> {
        Ok(sqlx::query_as("SELECT * FROM repo_disc WHERE album_id = ? AND disc_id = ?")
            .bind(album_id)
            .bind(disc_id)
            .fetch_optional(&self.pool)
            .await?)
    }

    pub async fn discs(&self, album_id: Uuid) -> RepoResult<Vec<rows::DiscRow>> {
        Ok(sqlx::query_as("SELECT * FROM repo_disc WHERE album_id = ? ORDER BY disc_id")
            .bind(album_id)
            .fetch_all(&self.pool)
            .await?)
    }

    pub async fn track(&self, album_id: Uuid, disc_id: u8, track_id: u8) -> RepoResult<Option<rows::TrackRow>> {
        Ok(sqlx::query_as("SELECT * FROM repo_track WHERE album_id = ? AND disc_id = ? AND track_id = ?")
            .bind(album_id)
            .bind(disc_id)
            .bind(track_id)
            .fetch_optional(&self.pool)
            .await?)
    }

    pub async fn tracks(&self, album_id: Uuid, disc_id: u8) -> RepoResult<Vec<rows::TrackRow>> {
        Ok(sqlx::query_as("SELECT * FROM repo_disc WHERE album_id = ? AND disc_id = ? ORDER BY track_id")
            .bind(album_id)
            .bind(disc_id)
            .fetch_all(&self.pool)
            .await?)
    }

    pub async fn reload(&mut self) -> RepoResult<()> {
        self.pool.close().await;
        self.pool = sqlx::SqlitePool::connect(&self.uri).await?;
        Ok(())
    }
}

pub struct RepoDatabaseWrite {
    conn: sqlx::SqliteConnection,
}

impl RepoDatabaseWrite {
    pub async fn create(path: impl AsRef<Path>) -> RepoResult<Self> {
        let conn = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Off)
            .synchronous(SqliteSynchronous::Off)
            .connect()
            .await?;
        let mut me = Self { conn };
        me.create_tables().await?;
        Ok(me)
    }

    async fn create_tables(&mut self) -> RepoResult<()> {
        sqlx::query(r#"
CREATE TABLE IF NOT EXISTS "repo_album" (
  "album_id"       BLOB NOT NULL UNIQUE,
  "title"          TEXT NOT NULL,
  "edition"        TEXT,
  "catalog"        TEXT NOT NULL,
  "artist"         TEXT NOT NULL,
  "release_date"   TEXT NOT NULL,
  "disc_count"     INTEGER NOT NULL,
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
  "track_count" INTEGER NOT NULL,
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
  "track_id"     INTEGER NOT NULL,
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
  "tag_id"      INTEGER NOT NULL UNIQUE,
  "name"        TEXT NOT NULL UNIQUE,
  PRIMARY KEY("tag_id" AUTOINCREMENT)
);
"#)
            .execute(&mut self.conn)
            .await?;

        sqlx::query(r#"
CREATE TABLE IF NOT EXISTS "repo_tag_detail" (
  "tag_id"      INTEGER NOT NULL,
  "album_id"    BLOB NOT NULL,
  "disc_id"     INTEGER,
  "track_id"    INTEGER,
  FOREIGN KEY("tag_id") REFERENCES "repo_tag"("tag_id")
);
"#)
            .execute(&mut self.conn)
            .await?;

        sqlx::query(r#"
CREATE TABLE IF NOT EXISTS "repo_tag_alias" (
  "tag_id"    INTEGER NOT NULL,
  "alias"     TEXT NOT NULL,
  FOREIGN KEY("tag_id") REFERENCES "repo_tag"("tag_id")
);
"#)
            .execute(&mut self.conn)
            .await?;

        sqlx::query(r#"
CREATE TABLE IF NOT EXISTS "repo_tag_relation" (
  "tag_id"      INTEGER NOT NULL,
  "parent_id"   INTEGER NOT NULL,
  FOREIGN KEY("tag_id") REFERENCES "repo_tag"("tag_id"),
  FOREIGN KEY("parent_id") REFERENCES "repo_tag"("tag_id")
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

    pub async fn create_index(&mut self) -> RepoResult<()> {
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
CREATE INDEX IF NOT EXISTS "repo_tag_detail_index" ON "repo_tag_detail" (
  "album_id",
  "disc_id",
  "track_id"
);
"#)
            .execute(&mut self.conn)
            .await?;
        Ok(())
    }

    pub async fn add_album(&mut self, album: &crate::models::Album) -> RepoResult<()> {
        let album_id = album.album_id();

        // add album info
        sqlx::query("INSERT INTO repo_album (album_id, title, edition, catalog, artist, release_date, disc_count, album_type) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(&album_id)
            .bind(album.title_raw())
            .bind(album.edition_raw())
            .bind(album.catalog())
            .bind(album.artist())
            .bind(album.release_date().to_string())
            .bind(album.discs().len() as i32)
            .bind(album.track_type().as_ref())
            .execute(&mut self.conn)
            .await?;

        // add album tags
        for tag in album.tags() {
            sqlx::query("INSERT INTO repo_tag_detail (album_id, tag_id) SELECT ?, tag_id FROM repo_tag WHERE name = ?")
                .bind(&album_id)
                .bind(tag.name())
                .execute(&mut self.conn)
                .await?;
        }

        for (disc_id, disc) in album.discs().iter().enumerate() {
            let disc_id = disc_id + 1;

            // add disc info
            sqlx::query("INSERT INTO repo_disc (album_id, disc_id, title, artist, catalog, track_count, disc_type) VALUES (?, ?, ?, ?, ?, ?, ?)")
                .bind(&album_id)
                .bind(disc_id as u8)
                .bind(disc.title())
                .bind(disc.artist())
                .bind(disc.catalog())
                .bind(disc.tracks().len() as i32)
                .bind(disc.track_type().as_ref())
                .execute(&mut self.conn)
                .await?;

            // add disc tags
            for tag in disc.tags() {
                sqlx::query("INSERT INTO repo_tag_detail (album_id, disc_id, tag_id) SELECT ?, ?, tag_id FROM repo_tag WHERE name = ?")
                    .bind(&album_id)
                    .bind(disc_id as u8)
                    .bind(tag.name())
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
                    sqlx::query("INSERT INTO repo_tag_detail (album_id, disc_id, track_id, tag_id) SELECT ?, ?, ?, tag_id FROM repo_tag WHERE name = ?")
                        .bind(&album_id)
                        .bind(disc_id as u8)
                        .bind(track_id as u8)
                        .bind(tag.name())
                        .execute(&mut self.conn)
                        .await?;
                }
            }
        }
        Ok(())
    }

    async fn add_tag(&mut self, name: &str) -> RepoResult<i32> {
        let tag_result = sqlx::query("INSERT INTO repo_tag (name) VALUES (?)")
            .bind(name)
            .execute(&mut self.conn)
            .await?;
        // this is a hack to get the id of the tag we just inserted
        let (id, ..) = sqlx::query_as::<_, (i32, i32)>("SELECT tag_id, 1 FROM repo_tag WHERE rowid = ?")
            .bind(tag_result.last_insert_rowid())
            .fetch_one(&mut self.conn)
            .await?;
        Ok(id)
    }

    async fn add_alias(&mut self, tag_id: i32, alias: &str) -> RepoResult<()> {
        sqlx::query("INSERT INTO repo_tag_alias (tag_id, alias) VALUES (?, ?)")
            .bind(tag_id)
            .bind(alias)
            .execute(&mut self.conn)
            .await?;
        Ok(())
    }

    async fn add_parent(&mut self, tag_id: i32, parent_id: i32) -> RepoResult<()> {
        sqlx::query("INSERT INTO repo_tag_relation (tag_id, parent_id) VALUES (?, ?)")
            .bind(tag_id)
            .bind(parent_id)
            .execute(&mut self.conn)
            .await?;
        Ok(())
    }

    pub async fn add_tags(&mut self, tags: impl Iterator<Item=&crate::models::Tag>) -> RepoResult<()> {
        let mut tag_id = HashMap::new();
        let mut relation_deferred = HashMap::new();

        for tag in tags {
            let id = self.add_tag(tag.name()).await?;
            tag_id.insert(tag.get_ref(), id);

            // add alias
            for alias in tag.alias() {
                self.add_alias(id, alias).await?;
            }

            // children are listed in tags now
            // add children
            // for child in tag.children_raw() {
            //     log::warn!("{}", child);
            //     let child_id = self.add_tag(child.name(), child.edition()).await?;
            //     tag_id.insert(child.clone(), child_id);
            //     self.add_parent(child_id, id).await?;
            // }

            // add parents to wait list
            relation_deferred.insert(id, tag.parents().iter());
        }

        for (child_id, parents) in relation_deferred {
            for parent in parents {
                self.add_parent(child_id, tag_id[parent]).await?;
            }
        }
        Ok(())
    }

    async fn add_info(&mut self, key: &str, value: &str) -> RepoResult<()> {
        sqlx::query("INSERT INTO repo_info (key, value) VALUES (?, ?)")
            .bind(key)
            .bind(value)
            .execute(&mut self.conn)
            .await?;
        Ok(())
    }

    pub async fn write_info(&mut self, repo_name: &str, repo_edition: &str, repo_url: &str, repo_ref: &str) -> RepoResult<()> {
        self.add_info("repo_name", repo_name).await?;
        self.add_info("repo_edition", repo_edition).await?;
        self.add_info("repo_url", repo_url).await?;
        self.add_info("repo_ref", repo_ref).await?;
        self.add_info("db_version", DB_VERSION).await?;
        Ok(())
    }

    pub async fn add_detailed_artist(&mut self, key: &str, value: &str) -> RepoResult<()> {
        sqlx::query("INSERT INTO repo_artists (key, value) VALUES (?, ?)")
            .bind(key)
            .bind(value)
            .execute(&mut self.conn)
            .await?;
        Ok(())
    }
}