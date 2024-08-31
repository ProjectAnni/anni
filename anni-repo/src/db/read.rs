use crate::db::rows;
use crate::prelude::RepoResult;
use anni_metadata::model::{
    Album, AlbumInfo, AnniDate, Disc, DiscInfo, TagString, TagType, Track, TrackType,
};
use rusqlite::{params, Connection, OpenFlags, Params};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_rusqlite::from_rows;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use uuid::Uuid;

pub struct RepoDatabaseRead {
    uri: PathBuf,
    conn: Connection,
}

#[derive(Serialize)]
pub struct TagEntry {
    pub tag: TagString,
    pub children: Vec<TagString>,
}

impl RepoDatabaseRead {
    pub fn new<P>(path: P) -> RepoResult<RepoDatabaseRead>
    where
        P: AsRef<Path>,
    {
        Ok(Self {
            uri: path.as_ref().to_path_buf(),
            conn: Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?,
        })
    }

    #[cfg(target_arch = "wasm32")]
    pub fn new_with_vfs(path: &str, vfs: &str) -> RepoResult<RepoDatabaseRead> {
        Ok(Self {
            uri: path.as_ref().to_path_buf(),
            conn: Connection::open_with_flags_and_vfs(path, OpenFlags::SQLITE_OPEN_READ_ONLY, vfs)?,
        })
    }

    pub fn match_album(
        &self,
        catalog: &str,
        release_date: &AnniDate,
        disc_count: u8,
        album_title: &str,
        _edition: Option<&str>,
    ) -> RepoResult<Option<Uuid>> {
        log::trace!("Catalog: {catalog}, Title: {album_title}, Release date: {release_date}, Discs: {disc_count}");
        let mut stmt = self.conn.prepare(
            "SELECT album_id, title FROM repo_album
  WHERE catalog = ? AND release_date = ? AND disc_count = ?;",
        )?;
        let albums_iter = stmt.query_map(
            params![catalog, release_date.to_string(), disc_count],
            |row| Ok((row.get(0)?, row.get(1)?)),
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

    #[doc(hidden)]
    pub fn query_optional<P, T>(&self, sql: &str, params: P) -> RepoResult<Option<T>>
    where
        P: Params,
        T: DeserializeOwned,
    {
        let mut stmt = self.conn.prepare(sql)?;
        let mut rows = from_rows::<T>(stmt.query(params)?);
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            _ => Ok(None),
        }
    }

    #[doc(hidden)]
    pub fn query_list<P, T>(&self, sql: &str, params: P) -> RepoResult<Vec<T>>
    where
        P: Params,
        T: DeserializeOwned,
    {
        let mut stmt = self.conn.prepare(sql)?;
        let rows = from_rows::<T>(stmt.query(params)?).collect::<Vec<_>>();
        if rows.is_empty() {
            Ok(Vec::new())
        } else {
            Ok(rows.into_iter().filter_map(|row| row.ok()).collect())
        }
    }

    /// Read a full `Album` from the database.
    ///
    /// The album is not formatted.
    pub fn read_album(&self, album_id: Uuid) -> RepoResult<Option<Album>> {
        let album_row = self.get_album(album_id)?;
        let album_row = match album_row {
            Some(album) => album,
            None => return Ok(None),
        };
        let album_tags = self.get_item_tags(album_id, None, None)?;
        let album_info = AlbumInfo {
            album_id,
            title: album_row.title,
            edition: album_row.edition,
            artist: album_row.artist,
            artists: None,
            release_date: AnniDate::from_str(&album_row.release_date)?,
            album_type: TrackType::from_str(&album_row.album_type)?,
            catalog: album_row.catalog,
            tags: album_tags,
        };

        let discs_row = self.get_discs(album_id)?;
        let mut discs = Vec::with_capacity(discs_row.len());
        for disc in discs_row {
            let disc_tags = self.get_item_tags(album_id, Some(disc.disc_id), None)?;
            let disc_info = DiscInfo::new(
                disc.catalog,
                Some(disc.title),
                Some(disc.artist),
                None,
                Some(TrackType::from_str(&disc.disc_type)?),
                disc_tags,
            );

            let tracks_row = self.get_tracks(album_id, disc.disc_id)?;
            let mut tracks = Vec::with_capacity(tracks_row.len());
            for track in tracks_row {
                let track_tags =
                    self.get_item_tags(album_id, Some(disc.disc_id), Some(track.track_id))?;
                let track = Track::new(
                    track.title,
                    Some(track.artist),
                    None,
                    Some(TrackType::from_str(&track.track_type)?),
                    track_tags,
                );
                tracks.push(track);
            }

            let disc = Disc::new(disc_info, tracks);
            discs.push(disc);
        }

        let album = Album::new(album_info, discs);
        Ok(Some(album))
    }

    pub fn get_album(&self, album_id: Uuid) -> RepoResult<Option<rows::AlbumRow>> {
        self.query_optional("SELECT * FROM repo_album WHERE album_id = ?", [album_id])
    }

    pub fn get_disc(&self, album_id: Uuid, disc_id: u8) -> RepoResult<Option<rows::DiscRow>> {
        self.query_optional(
            "SELECT * FROM repo_disc WHERE album_id = ? AND disc_id = ?",
            params![album_id, disc_id],
        )
    }

    pub fn get_discs(&self, album_id: Uuid) -> RepoResult<Vec<rows::DiscRow>> {
        self.query_list(
            "SELECT * FROM repo_disc WHERE album_id = ? ORDER BY disc_id",
            params![album_id],
        )
    }

    pub fn get_track(
        &self,
        album_id: Uuid,
        disc_id: u8,
        track_id: u8,
    ) -> RepoResult<Option<rows::TrackRow>> {
        self.query_optional(
            "SELECT * FROM repo_track WHERE album_id = ? AND disc_id = ? AND track_id = ?",
            params![album_id, disc_id, track_id],
        )
    }

    pub fn get_tracks(&self, album_id: Uuid, disc_id: u8) -> RepoResult<Vec<rows::TrackRow>> {
        self.query_list(
            "SELECT * FROM repo_track WHERE album_id = ? AND disc_id = ? ORDER BY track_id",
            params![album_id, disc_id],
        )
    }

    #[deprecated = "Use `get_item_tags` instead"]
    pub fn get_tag(
        &self,
        album_id: Uuid,
        disc_id: Option<u8>,
        track_id: Option<u8>,
    ) -> RepoResult<Vec<TagString>> {
        self.get_item_tags(album_id, disc_id, track_id)
    }

    /// Get a list of tags for an album, a disc, or a track.
    pub fn get_item_tags(
        &self,
        album_id: Uuid,
        disc_id: Option<u8>,
        track_id: Option<u8>,
    ) -> RepoResult<Vec<TagString>> {
        #[derive(Deserialize)]
        struct SimpleTagRow {
            name: String,
            tag_type: String,
        }

        let tags: Vec<SimpleTagRow>=
            match (disc_id, track_id) {
                (None, None) => self.query_list("SELECT * FROM repo_tag WHERE tag_id IN (SELECT tag_id FROM repo_tag_detail WHERE album_id = ? AND disc_id IS NULL AND track_id IS NULL)", params![album_id])?,
                (Some(disc_id), None) => self.query_list("SELECT * FROM repo_tag WHERE tag_id IN (SELECT tag_id FROM repo_tag_detail WHERE album_id = ? AND disc_id = ? AND track_id IS NULL)", params![album_id, disc_id])?,
                (Some(disc_id), Some(track_id)) => self.query_list("SELECT * FROM repo_tag WHERE tag_id IN (SELECT tag_id FROM repo_tag_detail WHERE album_id = ? AND disc_id = ? AND track_id = ?)", params![album_id, disc_id, track_id])?,
                _ => unreachable!(),
            };
        let mut result = Vec::with_capacity(tags.len());
        for tag in tags {
            let tag_type = TagType::from_str(&tag.tag_type)?;
            let tag = TagString::new(tag.name, tag_type);
            result.push(tag);
        }
        Ok(result)
    }

    /// Get relationship between tags
    pub fn get_tags_relationship(&self) -> RepoResult<HashMap<TagString, TagEntry>> {
        #[derive(Deserialize)]
        struct TagRelationRow {
            tag_id: u32,
            name: String,
            tag_type: TagType,
            children: Option<String>, // tag_id list concatenated by `,` or None
        }

        struct TagRelationRowOptimized {
            id: u32,
            tag: TagString,
            children: Vec<u32>,
        }

        let tags: Vec<TagRelationRow>= self.query_list("SELECT tag_id, name, tag_type, children FROM repo_tag LEFT JOIN (SELECT parent_id, group_concat(tag_id) children FROM repo_tag_relation GROUP BY parent_id) ON repo_tag.tag_id = parent_id", ())?;
        let tags: HashMap<_, _> = tags
            .into_iter()
            .map(|row| {
                (
                    row.tag_id,
                    TagRelationRowOptimized {
                        id: row.tag_id,
                        tag: TagString::new(row.name, row.tag_type),
                        children: row
                            .children
                            .map(|s| s.split(',').map(|s| s.parse().unwrap()).collect())
                            .unwrap_or_default(),
                    },
                )
            })
            .collect();

        let mut result = HashMap::new();
        for tag in tags.values() {
            let tag_info = tags.get(&tag.id).unwrap().tag.clone();
            let children = tag
                .children
                .iter()
                .map(|child_id| tags.get(child_id).unwrap().tag.clone())
                .collect();
            let entry = TagEntry {
                tag: tag_info,
                children,
            };
            result.insert(entry.tag.clone(), entry);
        }

        Ok(result)
    }

    pub fn get_albums_by_tag(&self, tag: &str, recursive: bool) -> RepoResult<Vec<rows::AlbumRow>> {
        if !recursive {
            self.query_list(
                r#"
SELECT * FROM repo_album WHERE album_id IN (
    SELECT DISTINCT album_id FROM repo_tag_detail WHERE tag_id = (
        SELECT tag_id FROM repo_tag WHERE name = ?
    )
)"#,
                params![tag],
            )
        } else {
            self.query_list(
                r#"
WITH RECURSIVE recursive_tags(tag_id) AS (
  SELECT tag_id FROM repo_tag WHERE name = ?

  UNION ALL

  SELECT rl.tag_id FROM repo_tag_relation rl, recursive_tags rt WHERE rl.parent_id = rt.tag_id
)

SELECT * FROM repo_album WHERE album_id IN (
    SELECT DISTINCT album_id FROM repo_tag_detail WHERE tag_id IN (
        SELECT * FROM recursive_tags
    )
)"#,
                params![tag],
            )
        }
    }

    pub fn reload(&mut self) -> RepoResult<()> {
        self.conn = Connection::open(&self.uri)?;
        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::rows::wasm::*;
    use super::*;
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;

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
            let album = self
                .db
                .get_album(Uuid::parse_str(&album_id).map_err(js_err)?)
                .map_err(js_err)?;
            Ok(serde_wasm_bindgen::to_value(&album)?.unchecked_into())
        }

        pub fn get_disc(&self, album_id: String, disc_id: u8) -> Result<IDiscRow, JsValue> {
            let album = self
                .db
                .get_disc(Uuid::parse_str(&album_id).map_err(js_err)?, disc_id)
                .map_err(js_err)?;
            Ok(serde_wasm_bindgen::to_value(&album)?.unchecked_into())
        }

        pub fn get_discs(&self, album_id: String) -> Result<IDiscRowArray, JsValue> {
            let album = self
                .db
                .get_discs(Uuid::parse_str(&album_id).map_err(js_err)?)
                .map_err(js_err)?;
            Ok(serde_wasm_bindgen::to_value(&album)?.unchecked_into())
        }

        pub fn get_track(
            &self,
            album_id: String,
            disc_id: u8,
            track_id: u8,
        ) -> Result<ITrackRow, JsValue> {
            let tracks = self
                .db
                .get_track(
                    Uuid::parse_str(&album_id).map_err(js_err)?,
                    disc_id,
                    track_id,
                )
                .map_err(js_err)?;
            Ok(serde_wasm_bindgen::to_value(&tracks)?.unchecked_into())
        }

        pub fn get_tracks(&self, album_id: String, disc_id: u8) -> Result<ITrackRowArray, JsValue> {
            let tracks = self
                .db
                .get_tracks(Uuid::parse_str(&album_id).map_err(js_err)?, disc_id)
                .map_err(js_err)?;
            Ok(serde_wasm_bindgen::to_value(&tracks)?.unchecked_into())
        }

        pub fn get_albums_by_tag(
            &self,
            tag: String,
            recursive: bool,
        ) -> Result<IAlbumRowArray, JsValue> {
            let tracks = self.db.get_albums_by_tag(&tag, recursive).map_err(js_err)?;
            Ok(serde_wasm_bindgen::to_value(&tracks)?.unchecked_into())
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn js_err<E: ToString>(e: E) -> JsValue {
        JsValue::from_str(&e.to_string())
    }
}

// #[cfg(test)]
// mod tests {
//     use crate::db::RepoDatabaseRead;
//     use crate::prelude::RepoResult;
//
//     // FIXME: fix this test
//     #[test]
//     fn test_read_album() -> RepoResult<()> {
//         let db = RepoDatabaseRead::new("/tmp/test/repo.db")?;
//         let tags = db.get_tags_relation()?;
//         Ok(())
//     }
// }
