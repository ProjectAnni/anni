use crate::db::DB_VERSION;
use crate::models::TagType;
use crate::prelude::RepoResult;
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::path::Path;

pub struct RepoDatabaseWrite {
    conn: Connection,
}

impl RepoDatabaseWrite {
    pub fn create(path: impl AsRef<Path>) -> RepoResult<Self> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "off")?;
        conn.pragma_update(None, "synchronous", "off")?;
        let me = Self { conn };
        me.create_tables()?;
        Ok(me)
    }

    fn create_tables(&self) -> RepoResult<()> {
        self.conn.execute_batch(r#"
BEGIN;

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

CREATE TABLE IF NOT EXISTS "repo_tag" (
  "tag_id"      INTEGER NOT NULL UNIQUE,
  "name"        TEXT NOT NULL UNIQUE,
  "tag_type"    TEXT NOT NULL DEFAULT 'default' CHECK("tag_type" IN ('artist', 'group', 'animation', 'series', 'project', 'game', 'organization', 'default', 'category')),
  PRIMARY KEY("tag_id" AUTOINCREMENT)
);

CREATE TABLE IF NOT EXISTS "repo_tag_detail" (
  "tag_id"      INTEGER NOT NULL,
  "album_id"    BLOB NOT NULL,
  "disc_id"     INTEGER,
  "track_id"    INTEGER,
  FOREIGN KEY("tag_id") REFERENCES "repo_tag"("tag_id")
);

CREATE TABLE IF NOT EXISTS "repo_tag_alias" (
  "tag_id"    INTEGER NOT NULL,
  "alias"     TEXT NOT NULL,
  FOREIGN KEY("tag_id") REFERENCES "repo_tag"("tag_id")
);

CREATE TABLE IF NOT EXISTS "repo_tag_relation" (
  "tag_id"      INTEGER NOT NULL,
  "parent_id"   INTEGER NOT NULL,
  FOREIGN KEY("tag_id") REFERENCES "repo_tag"("tag_id"),
  FOREIGN KEY("parent_id") REFERENCES "repo_tag"("tag_id")
);

CREATE TABLE IF NOT EXISTS "repo_info" (
  "key"    TEXT NOT NULL UNIQUE,
  "value"  TEXT
);

COMMIT;"#)?;
        Ok(())
    }

    pub fn create_index(&self) -> RepoResult<()> {
        self.conn.execute_batch(
            r#"
BEGIN;

CREATE UNIQUE INDEX "repo_album_index" ON "repo_album" (
  "album_id"
);

CREATE UNIQUE INDEX IF NOT EXISTS "repo_disc_index" ON "repo_disc" (
  "album_id",
  "disc_id"
);

CREATE UNIQUE INDEX IF NOT EXISTS "repo_track_index" ON "repo_track" (
  "album_id",
  "disc_id",
  "track_id"
);

CREATE INDEX IF NOT EXISTS "repo_tag_detail_index" ON "repo_tag_detail" (
  "album_id",
  "disc_id",
  "track_id"
);

COMMIT;
"#,
        )?;

        Ok(())
    }

    pub fn add_album(&self, album: &crate::models::Album) -> RepoResult<()> {
        let album_id = album.album_id();

        // add album info
        self.conn.execute(
            "INSERT INTO repo_album (album_id, title, edition, catalog, artist, release_date, disc_count, album_type) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                album_id,
                album.title_raw(),
                album.edition_raw(),
                album.catalog(),
                album.artist(),
                album.release_date().to_string(),
                album.discs().len(),
                album.track_type().as_ref(),
            ],
        )?;

        // add album tags
        for tag in album.tags_raw() {
            self.conn.execute(
                "INSERT INTO repo_tag_detail (album_id, tag_id) SELECT ?, tag_id FROM repo_tag WHERE name = ?",
                params![
                    album_id,
                    tag.name(),
                ],
            )?;
        }

        for (disc_id, disc) in album.discs().iter().enumerate() {
            let disc_id = disc_id + 1;

            // add disc info
            self.conn.execute(
                "INSERT INTO repo_disc (album_id, disc_id, title, artist, catalog, track_count, disc_type) VALUES (?, ?, ?, ?, ?, ?, ?)",
                params![
                    album_id,
                    disc_id,
                    disc.title(),
                    disc.artist(),
                    disc.catalog(),
                    disc.tracks().len(),
                    disc.track_type().as_ref(),
                ],
            )?;

            // add disc tags
            for tag in disc.tags() {
                self.conn.execute(
                    "INSERT INTO repo_tag_detail (album_id, disc_id, tag_id) SELECT ?, ?, tag_id FROM repo_tag WHERE name = ?",
                    params![
                        album_id,
                        disc_id,
                        tag.name(),
                    ],
                )?;
            }

            for (track_id, track) in disc.tracks().iter().enumerate() {
                let track_id = track_id + 1;

                // add track info
                self.conn.execute(
                    "INSERT INTO repo_track (album_id, disc_id, track_id, title, artist, track_type) VALUES (?, ?, ?, ?, ?, ?)",
                    params![
                        album_id,
                        disc_id,
                        track_id,
                        track.title(),
                        track.artist(),
                        track.track_type().as_ref(),
                    ],
                )?;

                // add track tags
                for tag in track.tags() {
                    self.conn.execute(
                        "INSERT INTO repo_tag_detail (album_id, disc_id, track_id, tag_id) SELECT ?, ?, ?, tag_id FROM repo_tag WHERE name = ?",
                        params![
                            album_id,
                            disc_id,
                            track_id,
                            tag.name(),
                        ],
                    )?;
                }
            }
        }
        Ok(())
    }

    fn add_tag(&self, name: &str, tag_type: &TagType) -> RepoResult<i32> {
        self.conn.execute(
            "INSERT INTO repo_tag (name, tag_type) VALUES (?, ?)",
            params![name, tag_type.to_string(),],
        )?;
        // this is a hack to get the id of the tag we just inserted
        let mut stmt = self
            .conn
            .prepare("SELECT tag_id FROM repo_tag WHERE rowid = ?")?;
        let id = stmt
            .query_map([self.conn.last_insert_rowid()], |row| Ok(row.get(0)?))?
            .next()
            .unwrap()?;
        Ok(id)
    }

    fn add_alias(&self, tag_id: i32, alias: &str) -> RepoResult<()> {
        self.conn.execute(
            "INSERT INTO repo_tag_alias (tag_id, alias) VALUES (?, ?)",
            params![tag_id, alias,],
        )?;
        Ok(())
    }

    fn add_parent(&self, tag_id: i32, parent_id: i32) -> RepoResult<()> {
        self.conn.execute(
            "INSERT INTO repo_tag_relation (tag_id, parent_id) VALUES (?, ?)",
            [tag_id, parent_id],
        )?;
        Ok(())
    }

    pub fn add_tags<'tag>(
        &self,
        tags: impl Iterator<Item = &'tag crate::models::Tag>,
    ) -> RepoResult<()> {
        let mut tag_id = HashMap::new();
        let mut relation_deferred = HashMap::new();

        for tag in tags {
            let id = self.add_tag(tag.name(), tag.tag_type())?;
            tag_id.insert(tag.get_ref(), id);

            // add alias
            for alias in tag.alias() {
                self.add_alias(id, alias)?;
            }

            // children are listed in tags now
            // add children
            // for child in tag.children_raw() {
            //     log::warn!("{}", child);
            //     let child_id = self.add_tag(child.name(), child.edition())?;
            //     tag_id.insert(child.clone(), child_id);
            //     self.add_parent(child_id, id)?;
            // }

            // add parents to wait list
            relation_deferred.insert(id, tag.parents().iter());
        }

        for (child_id, parents) in relation_deferred {
            for parent in parents {
                self.add_parent(child_id, tag_id[parent])?;
            }
        }
        Ok(())
    }

    fn add_info(&self, key: &str, value: &str) -> RepoResult<()> {
        self.conn.execute(
            "INSERT INTO repo_info (key, value) VALUES (?, ?)",
            [key, value],
        )?;
        Ok(())
    }

    pub fn write_info(
        &self,
        repo_name: &str,
        repo_edition: &str,
        repo_url: &str,
        repo_ref: &str,
    ) -> RepoResult<()> {
        self.add_info("repo_name", repo_name)?;
        self.add_info("repo_edition", repo_edition)?;
        self.add_info("repo_url", repo_url)?;
        self.add_info("repo_ref", repo_ref)?;
        self.add_info("db_version", DB_VERSION)?;
        Ok(())
    }

    pub fn add_detailed_artist(&self, key: &str, value: &str) -> RepoResult<()> {
        self.conn.execute(
            "INSERT INTO repo_artists (key, value) VALUES (?, ?)",
            [key, value],
        )?;
        Ok(())
    }
}
