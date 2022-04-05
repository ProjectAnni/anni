use rusqlite::{Connection, params, Params};
use serde::de::DeserializeOwned;
use serde_rusqlite::from_rows;
use uuid::Uuid;
use crate::db::rows;
use crate::prelude::RepoResult;

pub struct RepoDatabaseRead {
    uri: String,
    conn: Connection,
}

impl RepoDatabaseRead {
    pub fn new(path: &str) -> RepoResult<Self> {
        Ok(Self {
            uri: path.to_string(),
            conn: Connection::open(path)?,
        })
    }

    pub fn match_album(&self, catalog: &str, release_date: &crate::models::AnniDate, disc_count: u8, album_title: &str) -> RepoResult<Option<Uuid>> {
        log::trace!("Catalog: {catalog}, Title: {album_title}, Release date: {release_date}, Discs: {disc_count}");
        let mut stmt = self.conn.prepare(
            "SELECT album_id, title FROM repo_album
  WHERE catalog = ? AND release_date = ? AND disc_count = ?;")?;
        let albums_iter = stmt.query_map(
            params![catalog, release_date.to_string(), disc_count],
            |row| {
                Ok((row.get(0)?, row.get(1)?))
            },
        )?;
        let mut albums: Vec<(Uuid, String)> = Vec::new();
        for album in albums_iter {
            albums.push(album?);
        }

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

    fn query_optional<P, T>(&self, sql: &str, params: P) -> RepoResult<Option<T>>
        where
            P: Params,
            T: DeserializeOwned, {
        let mut stmt = self.conn.prepare(sql)?;
        let mut rows = from_rows::<T>(stmt.query(params)?).collect::<Vec<_>>();
        if rows.is_empty() {
            Ok(None)
        } else {
            let row = rows.pop().unwrap()?;
            Ok(Some(row))
        }
    }

    fn query_list<P, T>(&self, sql: &str, params: P) -> RepoResult<Vec<T>>
        where
            P: Params,
            T: DeserializeOwned, {
        let mut stmt = self.conn.prepare(sql)?;
        let rows = from_rows::<T>(stmt.query(params)?).collect::<Vec<_>>();
        if rows.is_empty() {
            Ok(Vec::new())
        } else {
            Ok(rows.into_iter().filter_map(|row| row.ok()).collect())
        }
    }

    pub fn album(&self, album_id: Uuid) -> RepoResult<Option<rows::AlbumRow>> {
        self.query_optional("SELECT * FROM repo_album WHERE album_id = ?", [album_id])
    }

    pub fn disc(&self, album_id: Uuid, disc_id: u8) -> RepoResult<Option<rows::DiscRow>> {
        self.query_optional("SELECT * FROM repo_disc WHERE album_id = ? AND disc_id = ?", params![album_id, disc_id])
    }

    pub fn discs(&self, album_id: Uuid) -> RepoResult<Vec<rows::DiscRow>> {
        self.query_list("SELECT * FROM repo_disc WHERE album_id = ? ORDER BY disc_id", [album_id])
    }

    pub fn track(&self, album_id: Uuid, disc_id: u8, track_id: u8) -> RepoResult<Option<rows::TrackRow>> {
        self.query_optional("SELECT * FROM repo_track WHERE album_id = ? AND disc_id = ? AND track_id = ?", params![album_id, disc_id, track_id])
    }

    pub fn tracks(&self, album_id: Uuid, disc_id: u8) -> RepoResult<Vec<rows::TrackRow>> {
        self.query_list("SELECT * FROM repo_track WHERE album_id = ? AND disc_id = ? ORDER BY track_id", params![album_id, disc_id])
    }

    pub fn reload(&mut self) -> RepoResult<()> {
        self.conn = Connection::open(&self.uri)?;
        Ok(())
    }
}
