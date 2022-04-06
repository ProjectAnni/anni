use rusqlite::{Connection, OpenFlags, params, Params};
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
    pub fn new(path: &str) -> RepoResult<RepoDatabaseRead> {
        Ok(Self {
            uri: path.to_string(),
            conn: Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?,
        })
    }

    #[cfg(target_arch = "wasm32")]
    pub fn new_with_vfs(path: &str, vfs: &str) -> RepoResult<RepoDatabaseRead> {
        Ok(Self {
            uri: path.to_string(),
            conn: Connection::open_with_flags_and_vfs(path, OpenFlags::SQLITE_OPEN_READ_ONLY, vfs)?,
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
        let mut rows = from_rows::<T>(stmt.query(params)?);
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            _ => Ok(None),
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

    pub fn get_album(&self, album_id: Uuid) -> RepoResult<Option<rows::AlbumRow>> {
        self.query_optional("SELECT * FROM repo_album WHERE album_id = ?", [album_id])
    }

    pub fn get_disc(&self, album_id: Uuid, disc_id: u8) -> RepoResult<Option<rows::DiscRow>> {
        self.query_optional("SELECT * FROM repo_disc WHERE album_id = ? AND disc_id = ?", params![album_id, disc_id])
    }

    pub fn get_discs(&self, album_id: Uuid) -> RepoResult<Vec<rows::DiscRow>> {
        self.query_list("SELECT * FROM repo_disc WHERE album_id = ? ORDER BY disc_id", params![album_id])
    }

    pub fn get_track(&self, album_id: Uuid, disc_id: u8, track_id: u8) -> RepoResult<Option<rows::TrackRow>> {
        self.query_optional("SELECT * FROM repo_track WHERE album_id = ? AND disc_id = ? AND track_id = ?", params![album_id, disc_id, track_id])
    }

    pub fn get_tracks(&self, album_id: Uuid, disc_id: u8) -> RepoResult<Vec<rows::TrackRow>> {
        self.query_list("SELECT * FROM repo_track WHERE album_id = ? AND disc_id = ? ORDER BY track_id", params![album_id, disc_id])
    }

    pub fn get_albums_by_tag(&self, tag: &str, recursive: bool) -> RepoResult<Vec<rows::AlbumRow>> {
        if !recursive {
            self.query_list(r#"
SELECT * FROM repo_album WHERE album_id IN (
    SELECT DISTINCT album_id FROM repo_tag_detail WHERE tag_id = (
        SELECT tag_id FROM repo_tag WHERE name = ?
    )
)"#, params![tag])
        } else {
            self.query_list(r#"
WITH RECURSIVE recursive_tags(tag_id) AS (
  SELECT tag_id FROM repo_tag WHERE name = ?

  UNION ALL

  SELECT rl.tag_id FROM repo_tag_relation rl, recursive_tags rt WHERE rl.parent_id = rt.tag_id
)

SELECT * FROM repo_album WHERE album_id IN (
    SELECT DISTINCT album_id FROM repo_tag_detail WHERE tag_id IN (
        SELECT * FROM recursive_tags
    )
)"#, params![tag])
        }
    }

    pub fn reload(&mut self) -> RepoResult<()> {
        self.conn = Connection::open(&self.uri)?;
        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use wasm_bindgen::JsCast;
    use super::*;
    use wasm_bindgen::prelude::*;
    use super::rows::wasm::*;

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen]
    pub struct WasmDatabaseRead {
        db: RepoDatabaseRead,
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen]
    impl WasmDatabaseRead {
        #[wasm_bindgen(constructor)]
        pub fn new(path: String, vfs: Option<String>) -> WasmDatabaseRead {
            let db = match vfs {
                Some(name) => RepoDatabaseRead::new_with_vfs(&path, &name).unwrap(),
                None => RepoDatabaseRead::new(&path).unwrap(),
            };

            WasmDatabaseRead { db }
        }

        pub fn get_album(&self, album_id: String) -> Result<IAlbumRow, JsValue> {
            let album = self.db.get_album(Uuid::parse_str(&album_id).map_err(js_err)?).map_err(js_err)?;
            Ok(serde_wasm_bindgen::to_value(&album)?.unchecked_into())
        }

        pub fn get_disc(&self, album_id: String, disc_id: u8) -> Result<IDiscRow, JsValue> {
            let album = self.db.get_disc(Uuid::parse_str(&album_id).map_err(js_err)?, disc_id).map_err(js_err)?;
            Ok(serde_wasm_bindgen::to_value(&album)?.unchecked_into())
        }

        pub fn get_discs(&self, album_id: String) -> Result<IDiscRowArray, JsValue> {
            let album = self.db.get_discs(Uuid::parse_str(&album_id).map_err(js_err)?).map_err(js_err)?;
            Ok(serde_wasm_bindgen::to_value(&album)?.unchecked_into())
        }

        pub fn get_track(&self, album_id: String, disc_id: u8, track_id: u8) -> Result<ITrackRow, JsValue> {
            let tracks = self.db.get_track(Uuid::parse_str(&album_id).map_err(js_err)?, disc_id, track_id).map_err(js_err)?;
            Ok(serde_wasm_bindgen::to_value(&tracks)?.unchecked_into())
        }

        pub fn get_tracks(&self, album_id: String, disc_id: u8) -> Result<ITrackRowArray, JsValue> {
            let tracks = self.db.get_tracks(Uuid::parse_str(&album_id).map_err(js_err)?, disc_id).map_err(js_err)?;
            Ok(serde_wasm_bindgen::to_value(&tracks)?.unchecked_into())
        }

        pub fn get_albums_by_tag(&self, tag: String, recursive: bool) -> Result<IAlbumRowArray, JsValue> {
            let tracks = self.db.get_albums_by_tag(&tag, recursive).map_err(js_err)?;
            Ok(serde_wasm_bindgen::to_value(&tracks)?.unchecked_into())
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn js_err<E: ToString>(e: E) -> JsValue {
        JsValue::from_str(&e.to_string())
    }
}
