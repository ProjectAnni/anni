use crate::prelude::*;
use indexmap::IndexSet;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::ops::{Deref, DerefMut};
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug)]
pub struct TrackIdentifier {
    pub album_id: Uuid,
    pub disc_id: u32,
    pub track_id: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Album {
    #[serde(rename = "album")]
    pub(crate) info: AlbumInfo,
    pub(crate) discs: Vec<Disc>,
}

impl Album {
    pub fn new(info: AlbumInfo, discs: Vec<Disc>) -> Self {
        let mut album = Album { info, discs };
        album.format();
        album
    }

    pub(crate) fn resolve_tags(&mut self, tags: &HashMap<String, HashMap<TagType, Tag>>) {
        self.info.tags.iter_mut().for_each(|tag| tag.resolve(tags));
        self.discs
            .iter_mut()
            .for_each(|disc| disc.resolve_tags(tags));
    }
}

impl FromStr for Album {
    type Err = Error;

    fn from_str(toml_str: &str) -> Result<Self, Self::Err> {
        let album: Album = toml::from_str(toml_str).map_err(|e| Error::TomlParseError {
            target: "Album",
            input: toml_str.to_string(),
            err: e,
        })?;

        Ok(album)
    }
}

impl Deref for Album {
    type Target = AlbumInfo;

    fn deref(&self) -> &Self::Target {
        &self.info
    }
}

impl DerefMut for Album {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.info
    }
}

impl Album {
    pub fn album_id(&self) -> Uuid {
        self.info.album_id
    }

    /// Only album title uses edition parameter.
    pub fn full_title(&self) -> Cow<str> {
        if let Some(edition) = &self.info.edition {
            Cow::Owned(format!("{}【{edition}】", self.info.title))
        } else {
            Cow::Borrowed(&self.info.title)
        }
    }

    pub fn title_raw(&self) -> &str {
        self.info.title.as_ref()
    }

    pub fn edition(&self) -> Option<&str> {
        self.info.edition.as_deref()
    }

    pub fn artist(&self) -> &str {
        self.info.artist.as_ref()
    }

    pub fn release_date(&self) -> &AnniDate {
        &self.info.release_date
    }

    pub fn track_type(&self) -> &TrackType {
        &self.info.album_type
    }

    pub fn catalog(&self) -> &str {
        self.info.catalog.as_ref()
    }

    pub fn tags<'me, 'tag>(&'me self) -> Vec<&'me TagRef<'tag>>
    where
        'tag: 'me,
    {
        let mut tags = Vec::new();
        tags.append(&mut self.info.tags.iter().collect::<Vec<_>>());
        for disc in self.discs.iter() {
            if !disc.tags.is_empty() {
                tags.append(&mut disc.tags.iter().collect::<Vec<_>>());
            }
            for track in disc.tracks.iter() {
                if !track.tags.is_empty() {
                    tags.append(&mut track.tags.iter().collect::<Vec<_>>());
                }
            }
        }
        tags.into_iter()
            .map(|t| &t.0)
            .collect::<IndexSet<_>>()
            .into_iter()
            .collect()
    }

    pub fn album_tags(&self) -> Vec<&TagRef> {
        self.info.tags.iter().map(|t| &t.0).collect()
    }

    pub fn discs_len(&self) -> usize {
        self.discs.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = DiscRef> {
        self.discs.iter().map(move |disc| DiscRef {
            album: &self.info,
            disc,
        })
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = DiscRefMut> {
        let album = &self.info;
        self.discs.iter_mut().map(move |disc| DiscRefMut {
            album,
            disc: &mut disc.info,
            tracks: &mut disc.tracks,
        })
    }

    pub fn format(&mut self) {
        self.iter_mut().for_each(|mut disc| disc.format());

        let disc_artist = self
            .iter()
            .map(|disc| disc.artist().to_string())
            .collect::<HashSet<_>>();
        if disc_artist.len() == 1
            && (self.artist == "UnknownArtist"
                || self.artist == "[Unknown Artist]"
                || &self.artist == disc_artist.iter().next().unwrap())
        {
            // all artists of the discs are the same, set all artists of discs to None
            for disc in self.discs.iter_mut() {
                disc.artist = None;
            }
            self.artist = disc_artist.into_iter().next().unwrap();
        } else {
            // not the same, set part of them to None
            let album_artist = self.artist.to_string();
            for disc in self.discs.iter_mut() {
                if disc.artist.as_deref() == Some(&album_artist) {
                    disc.artist = None;
                }
            }
        }

        let album_type = self.album_type.clone();
        let all_discs_type = self
            .discs
            .iter()
            .map(|disc| disc.disc_type.as_ref().unwrap_or(&album_type))
            .collect::<HashSet<_>>();
        if all_discs_type.len() == 1 {
            let all_discs_type = all_discs_type.into_iter().next().unwrap();
            if &album_type != all_discs_type {
                // not the same, set album type
                self.album_type = all_discs_type.clone()
            }
            // all discs have the same type, set all discs' type to None
            for disc in self.discs.iter_mut() {
                disc.disc_type = None;
            }
        } else {
            // not the same, set part of them to None
            for disc in self.discs.iter_mut() {
                if disc.disc_type.as_ref() == Some(&album_type) {
                    disc.disc_type = None;
                }
            }
        }
    }

    pub fn format_to_string(&mut self) -> String {
        self.format();
        toml::to_string_pretty(&self).unwrap()
    }

    /// Apply album metadata to a directory formatted with strict album format.
    ///
    /// This function applies both metadata and cover.
    #[cfg(feature = "apply")]
    pub fn apply_strict<P>(&self, directory: P) -> Result<(), crate::error::AlbumApplyError>
    where
        P: AsRef<std::path::Path>,
    {
        use crate::error::AlbumApplyError;
        use anni_common::fs;
        use anni_flac::{
            blocks::{BlockPicture, PictureType, UserComment, UserCommentExt},
            FlacHeader, MetadataBlock, MetadataBlockData,
        };

        // check disc name
        let mut discs = fs::read_dir(directory.as_ref())?
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                entry
                    .metadata()
                    .ok()
                    .and_then(|meta| if meta.is_dir() { Some(entry) } else { None })
            })
            .filter_map(|entry| {
                entry
                    .path()
                    .file_name()
                    .and_then(|f| f.to_str().map(|s| s.to_string()))
            })
            .collect::<Vec<_>>();
        alphanumeric_sort::sort_str_slice(&mut discs);

        if self.discs_len() != discs.len() {
            return Err(AlbumApplyError::DiscMismatch {
                path: directory.as_ref().to_path_buf(),
                expected: self.discs_len(),
                actual: discs.len(),
            });
        }

        let album_cover_path = directory.as_ref().join("cover.jpg");
        if !album_cover_path.exists() {
            return Err(AlbumApplyError::MissingCover(album_cover_path));
        }

        for (index, disc_id) in discs.iter().enumerate() {
            let disc_path = directory.as_ref().join(disc_id);
            if disc_id != &(index + 1).to_string() {
                return Err(AlbumApplyError::InvalidDiscFolder(disc_path));
            }

            let disc_cover_path = disc_path.join("cover.jpg");
            if !disc_cover_path.exists() {
                return Err(AlbumApplyError::MissingCover(disc_cover_path));
            }
        }

        let disc_total = discs.len();

        for ((disc_id, disc), disc_name) in self.iter().enumerate().zip(discs) {
            let disc_num = disc_id + 1;
            let disc_dir = directory.as_ref().join(disc_name);

            let mut files = fs::get_ext_files(&disc_dir, "flac", false)?;
            alphanumeric_sort::sort_path_slice(&mut files);
            let tracks = disc.iter();
            let track_total = disc.tracks_len();

            if files.len() != track_total {
                return Err(AlbumApplyError::TrackMismatch {
                    path: disc_dir,
                    expected: track_total,
                    actual: files.len(),
                });
            }

            for (track_num, (file, track)) in files.iter().zip(tracks).enumerate() {
                let track_num = track_num + 1;

                let mut flac = FlacHeader::from_file(file)?;
                let comments = flac.comments();
                let meta = format!(
                    r#"TITLE={title}
    ALBUM={album}
    ARTIST={artist}
    DATE={release_date}
    TRACKNUMBER={track_number}
    TRACKTOTAL={track_total}
    DISCNUMBER={disc_number}
    DISCTOTAL={disc_total}
    "#,
                    title = track.title(),
                    album = disc.title(),
                    artist = track.artist(),
                    release_date = self.release_date(),
                    track_number = track_num,
                    disc_number = disc_num,
                );

                // let mut modified = false;
                // no comment block exist, or comments is not correct
                if comments.is_none() || comments.unwrap().to_string() != meta {
                    let comments = flac.comments_mut();
                    comments.clear();
                    comments.push(UserComment::title(track.title()));
                    comments.push(UserComment::album(disc.title()));
                    comments.push(UserComment::artist(track.artist()));
                    comments.push(UserComment::date(self.release_date()));
                    comments.push(UserComment::track_number(track_num));
                    comments.push(UserComment::track_total(track_total));
                    comments.push(UserComment::disc_number(disc_num));
                    comments.push(UserComment::disc_total(disc_total));
                    // modified = true;
                }

                // TODO: do not modify flac file if embed cover is the same as the one in folder
                let cover_path = file.with_file_name("cover.jpg");
                let picture =
                    BlockPicture::new(cover_path, PictureType::CoverFront, String::new())?;
                flac.blocks
                    .retain(|block| !matches!(block.data, MetadataBlockData::Picture(_)));
                flac.blocks
                    .push(MetadataBlock::new(MetadataBlockData::Picture(picture)));
                // modified = true;

                // if modified {
                flac.save::<String>(None)?;
                // }
            }
        }
        Ok(())
    }

    /// Apply album metadata to a directory formatted with **convention album format**.
    ///
    /// This function applies metadata only. Cover is not checked
    #[cfg(feature = "apply")]
    pub fn apply_convention<P>(&self, directory: P) -> Result<(), crate::error::AlbumApplyError>
    where
        P: AsRef<std::path::Path>,
    {
        use crate::error::AlbumApplyError;
        use anni_common::fs;
        use anni_flac::{
            blocks::{UserComment, UserCommentExt},
            FlacHeader,
        };

        let disc_total = self.discs_len();

        for (disc_num, disc) in self.iter().enumerate() {
            let disc_num = disc_num + 1;
            let disc_dir = if disc_total > 1 {
                directory.as_ref().join(format!(
                    "[{catalog}] {title} [Disc {disc_num}]",
                    catalog = disc.catalog(),
                    title = disc.title(),
                    disc_num = disc_num,
                ))
            } else {
                directory.as_ref().to_owned()
            };

            if !disc_dir.exists() {
                return Err(AlbumApplyError::InvalidDiscFolder(disc_dir));
            }

            let files = fs::get_ext_files(&disc_dir, "flac", false)?;
            let tracks = disc.iter();
            let track_total = disc.tracks_len();
            if files.len() != track_total {
                return Err(AlbumApplyError::TrackMismatch {
                    path: disc_dir,
                    expected: track_total,
                    actual: files.len(),
                });
            }

            for (track_num, (file, track)) in files.iter().zip(tracks).enumerate() {
                let track_num = track_num + 1;

                let mut flac = FlacHeader::from_file(file)?;
                let comments = flac.comments();
                // TODO: read anni convention config here
                let meta = format!(
                    r#"TITLE={title}
ALBUM={album}
ARTIST={artist}
DATE={release_date}
TRACKNUMBER={track_number}
TRACKTOTAL={track_total}
DISCNUMBER={disc_number}
DISCTOTAL={disc_total}
"#,
                    title = track.title(),
                    album = disc.title(),
                    artist = track.artist(),
                    release_date = self.release_date(),
                    track_number = track_num,
                    track_total = track_total,
                    disc_number = disc_num,
                    disc_total = disc_total,
                );
                // no comment block exist, or comments is not correct
                if comments.is_none() || comments.unwrap().to_string() != meta {
                    let comments = flac.comments_mut();
                    comments.clear();
                    comments.push(UserComment::title(track.title()));
                    comments.push(UserComment::album(disc.title()));
                    comments.push(UserComment::artist(track.artist()));
                    comments.push(UserComment::date(self.release_date()));
                    comments.push(UserComment::track_number(track_num));
                    comments.push(UserComment::track_total(track_total));
                    comments.push(UserComment::disc_number(disc_num));
                    comments.push(UserComment::disc_total(disc_total));
                    flac.save::<String>(None)?;
                }
            }
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct AlbumInfo {
    /// Album ID(uuid)
    pub album_id: Uuid,
    /// Album title
    pub title: String,
    /// Album edition
    ///
    /// If this field is not None and is not empty, the full title of Album should be {title}【{edition}】
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "anni_common::decode::non_empty_str")]
    pub edition: Option<String>,
    /// Album artist
    pub artist: String,
    /// Album artists
    #[serde(default)]
    #[serde(skip_serializing_if = "is_artists_empty")]
    pub artists: Option<HashMap<String, String>>,
    /// Album release date
    #[serde(rename = "date")]
    pub release_date: AnniDate,
    /// Album track type
    #[serde(rename = "type")]
    pub album_type: TrackType,
    /// Album catalog
    pub catalog: String,
    /// Album tags
    #[serde(default)]
    // TODO: use IndexSet
    pub tags: Vec<TagString>,
}

impl Default for AlbumInfo {
    fn default() -> Self {
        Self {
            album_id: Uuid::new_v4(),
            title: "UnknownTitle".to_string(),
            edition: None,
            artist: "[Unknown Artist]".to_string(),
            artists: HashMap::new().into(),
            release_date: AnniDate::new(2021, 1, 1),
            album_type: TrackType::Normal,
            catalog: "@TEMP".to_string(),
            tags: Default::default(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Disc {
    #[serde(flatten)]
    info: DiscInfo,
    tracks: Vec<Track>,
}

impl Disc {
    pub fn new(info: DiscInfo, tracks: Vec<Track>) -> Self {
        Self { info, tracks }
    }

    pub(crate) fn resolve_tags(&mut self, tags: &HashMap<String, HashMap<TagType, Tag>>) {
        self.tags.iter_mut().for_each(|tag| tag.resolve(tags));
        self.tracks
            .iter_mut()
            .for_each(|track| track.resolve_tags(tags));
    }
}

impl Deref for Disc {
    type Target = DiscInfo;

    fn deref(&self) -> &Self::Target {
        &self.info
    }
}

impl DerefMut for Disc {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.info
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq))]
#[serde(deny_unknown_fields)]
pub struct DiscInfo {
    /// Disc title
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    /// Disc catalog
    pub catalog: String,
    /// Disc artist
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    /// Disc artists
    #[serde(skip_serializing_if = "is_artists_empty")]
    pub artists: Option<HashMap<String, String>>,
    /// Disc type
    #[serde(rename = "type")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disc_type: Option<TrackType>,
    /// Disc tags
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<TagString>,
}

impl DiscInfo {
    pub fn new(
        catalog: String,
        title: Option<String>,
        artist: Option<String>,
        artists: Option<HashMap<String, String>>,
        disc_type: Option<TrackType>,
        tags: Vec<TagString>,
    ) -> Self {
        DiscInfo {
            title,
            artist,
            artists,
            catalog,
            tags,
            disc_type,
        }
    }
}

#[derive(Clone)]
pub struct DiscRef<'album> {
    pub(crate) album: &'album AlbumInfo,
    pub(crate) disc: &'album Disc,
}

impl<'album> DiscRef<'album> {
    pub fn title(&self) -> &str {
        self.disc
            .title
            .as_deref()
            .unwrap_or(self.album.title.as_str())
    }

    /// Get raw disc title without inherit
    pub fn title_raw(&self) -> Option<&str> {
        self.disc.title.as_deref()
    }

    pub fn artist(&self) -> &str {
        self.disc
            .artist
            .as_deref()
            .unwrap_or(self.album.artist.as_str())
    }

    /// Get raw disc artist without inherit
    pub fn artist_raw(&self) -> Option<&str> {
        self.disc.artist.as_deref()
    }

    pub fn artists(&self) -> Option<&HashMap<String, String>> {
        self.disc.artists.as_ref()
    }

    pub fn catalog(&self) -> &str {
        self.disc.catalog.as_ref()
    }

    pub fn track_type(&self) -> &TrackType {
        self.disc
            .disc_type
            .as_ref()
            .unwrap_or(&self.album.album_type)
    }

    pub fn tags_iter(&self) -> impl Iterator<Item = &TagRef> {
        self.disc.tags.iter().map(|t| &t.0)
    }

    pub fn tracks_len(&self) -> usize {
        self.disc.tracks.len()
    }

    pub fn raw(&self) -> &'album Disc {
        self.disc
    }

    pub fn iter<'disc>(&'disc self) -> impl Iterator<Item = TrackRef<'album, 'disc>> {
        self.disc.tracks.iter().map(move |track| TrackRef {
            album: self.album,
            disc: self.disc,
            track,
        })
    }
}

pub struct DiscRefMut<'album> {
    pub(crate) album: &'album AlbumInfo,
    pub(crate) disc: &'album mut DiscInfo,
    pub(crate) tracks: &'album mut Vec<Track>,
}

impl<'album> DiscRefMut<'album> {
    pub fn title(&self) -> &str {
        self.disc
            .title
            .as_deref()
            .unwrap_or(self.album.title.as_str())
    }

    pub fn artist(&self) -> &str {
        self.disc
            .artist
            .as_deref()
            .unwrap_or(self.album.artist.as_str())
    }

    pub fn catalog(&self) -> &str {
        self.disc.catalog.as_ref()
    }

    pub fn track_type(&self) -> &TrackType {
        self.disc
            .disc_type
            .as_ref()
            .unwrap_or(&self.album.album_type)
    }

    pub fn tags_iter(&self) -> impl Iterator<Item = &TagRef> {
        self.disc.tags.iter().map(|t| &t.0)
    }

    pub fn tracks_len(&self) -> usize {
        self.tracks.len()
    }

    pub fn iter<'disc>(&'disc self) -> impl Iterator<Item = TrackRef<'album, 'disc>> {
        self.tracks.iter().map(move |track| TrackRef {
            album: self.album,
            disc: self.disc,
            track,
        })
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = TrackRefMut> {
        let album = self.album;
        let disc = &self.disc;
        self.tracks
            .iter_mut()
            .map(move |track| TrackRefMut { album, disc, track })
    }

    pub fn format(&mut self) {
        // format artists
        let track_artist = self
            .iter()
            .map(|disc| disc.artist().to_string())
            .collect::<HashSet<_>>();
        if track_artist.len() == 1 {
            // all track artists are the same, set all of them to None
            for mut track in self.iter_mut() {
                track.artist = None;
            }
            self.disc.artist = Some(track_artist.into_iter().next().unwrap());
        } else {
            // not the same, ignore extraction
        }

        // format type
        // if all type of the tracks are the same, set the disc type to the same
        // or, re-use disc type to format part of tracks
        let disc_type = self.track_type().clone();
        let all_tracks_type = self
            .iter()
            .map(|track| track.track_type())
            .collect::<HashSet<_>>();
        if all_tracks_type.len() == 1 {
            let all_tracks_type = all_tracks_type.into_iter().next().unwrap();
            if &disc_type != all_tracks_type {
                self.disc.disc_type = Some(all_tracks_type.clone());
            }

            // set all tracks type to None
            for mut track in self.iter_mut() {
                track.track_type = None;
            }
        } else {
            for mut track in self.iter_mut() {
                if track.track_type() == &disc_type {
                    track.track_type = None;
                }
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Track {
    /// Track title
    pub title: String,
    /// Track artist
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    /// Track artists
    #[serde(skip_serializing_if = "is_artists_empty")]
    pub artists: Option<HashMap<String, String>>,
    /// Track type
    #[serde(rename = "type")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_type: Option<TrackType>,
    /// Track tags
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<TagString>,
}

impl Track {
    pub fn new(
        title: String,
        artist: Option<String>,
        artists: Option<HashMap<String, String>>,
        track_type: Option<TrackType>,
        tags: Vec<TagString>,
    ) -> Self {
        Track {
            title,
            artist,
            artists,
            track_type,
            tags,
        }
    }

    pub fn empty() -> Self {
        Track::new(String::new(), None, None, None, Default::default())
    }

    pub(crate) fn resolve_tags(&mut self, tags: &HashMap<String, HashMap<TagType, Tag>>) {
        self.tags.iter_mut().for_each(|tag| tag.resolve(tags));
    }
}

#[derive(Clone)]
pub struct TrackRef<'album, 'disc> {
    pub(crate) album: &'album AlbumInfo,
    pub(crate) disc: &'disc DiscInfo,
    pub(crate) track: &'disc Track,
}

impl<'album, 'disc> TrackRef<'album, 'disc>
where
    'album: 'disc,
{
    pub fn title(&self) -> &'disc str {
        self.track.title.as_ref()
    }

    pub fn artist(&self) -> &'disc str {
        self.track.artist.as_deref().unwrap_or_else(|| {
            self.disc
                .artist
                .as_deref()
                .unwrap_or(self.album.artist.as_str())
        })
    }

    pub fn artists(&self) -> Option<&'disc HashMap<String, String>> {
        self.track
            .artists
            .as_ref()
            .or_else(|| self.disc.artists.as_ref().or(self.album.artists.as_ref()))
    }

    pub fn track_type(&self) -> &'disc TrackType {
        self.track.track_type.as_ref().unwrap_or_else(|| {
            self.disc
                .disc_type
                .as_ref()
                .unwrap_or(&self.album.album_type)
        })
    }

    pub fn tags_iter<'me, 'tag>(&'me self) -> impl Iterator<Item = &'me TagRef<'tag>>
    where
        'tag: 'me,
    {
        self.track.tags.iter().map(|t| &t.0)
    }

    pub fn raw(&self) -> &'disc Track {
        self.track
    }
}

pub struct TrackRefMut<'album, 'disc> {
    pub(crate) album: &'album AlbumInfo,
    pub(crate) disc: &'disc DiscInfo,
    pub(crate) track: &'disc mut Track,
}

impl Deref for TrackRefMut<'_, '_> {
    type Target = Track;

    fn deref(&self) -> &Self::Target {
        self.track
    }
}

impl DerefMut for TrackRefMut<'_, '_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.track
    }
}

impl<'album, 'disc> TrackRefMut<'album, 'disc>
where
    'album: 'disc,
{
    fn inner(&'album self) -> TrackRef<'album, 'disc> {
        TrackRef {
            album: self.album,
            disc: self.disc,
            track: self.track,
        }
    }

    pub fn title(&self) -> &str {
        self.inner().title()
    }

    pub fn artist(&self) -> &str {
        self.inner().artist()
    }

    pub fn artists(&self) -> Option<&HashMap<String, String>> {
        self.inner().artists()
    }

    pub fn track_type(&self) -> &TrackType {
        self.inner().track_type()
    }

    pub fn tags_iter<'me, 'tag>(&'me self) -> impl Iterator<Item = &'me TagRef<'tag>>
    where
        'tag: 'me,
    {
        self.track.tags.iter().map(|t| &t.0)
    }

    pub fn set_artist(&mut self, artist: Option<String>) {
        if let Some(artist) = artist {
            let artist_str = artist.as_str();
            let current_artist_str = self.track.artist.as_deref().unwrap_or_else(|| {
                self.disc
                    .artist
                    .as_deref()
                    .unwrap_or(self.album.artist.as_str())
            });

            if artist_str == current_artist_str {
                self.track.artist = None;
            } else {
                self.track.artist = Some(artist);
            }
        } else {
            self.track.artist = None;
        }
    }

    pub fn set_track_type(&mut self, track_type: Option<TrackType>) {
        self.track.track_type = track_type;
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "lowercase")]
pub enum TrackType {
    Normal,
    Instrumental,
    Absolute,
    Drama,
    Radio,
    Vocal,
}

impl AsRef<str> for TrackType {
    fn as_ref(&self) -> &str {
        match &self {
            TrackType::Normal => "normal",
            TrackType::Instrumental => "instrumental",
            TrackType::Absolute => "absolute",
            TrackType::Drama => "drama",
            TrackType::Radio => "radio",
            TrackType::Vocal => "vocal",
        }
    }
}

impl FromStr for TrackType {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "normal" => Ok(TrackType::Normal),
            "instrumental" => Ok(TrackType::Instrumental),
            "absolute" => Ok(TrackType::Absolute),
            "drama" => Ok(TrackType::Drama),
            "radio" => Ok(TrackType::Radio),
            "vocal" => Ok(TrackType::Vocal),
            _ => Err(Error::InvalidTrackType(s.to_string())),
        }
    }
}

impl TrackType {
    pub fn guess(title: &str) -> Option<TrackType> {
        let title_lowercase = title.to_lowercase();
        if title_lowercase.contains("off vocal")
            || title_lowercase.contains("instrumental")
            || title_lowercase.contains("カラオケ")
            || title_lowercase.contains("offvocal")
        {
            Some(TrackType::Instrumental)
        } else if title_lowercase.contains("drama") || title_lowercase.contains("ドラマ") {
            Some(TrackType::Drama)
        } else if title_lowercase.contains("radio") || title_lowercase.contains("ラジオ") {
            Some(TrackType::Radio)
        } else {
            None
        }
    }
}

pub(crate) fn is_artists_empty(artists: &Option<HashMap<String, String>>) -> bool {
    match artists {
        Some(artists) => artists.is_empty(),
        None => true,
    }
}

#[cfg(feature = "flac")]
impl From<anni_flac::FlacHeader> for Track {
    fn from(stream: anni_flac::FlacHeader) -> Self {
        use regex::Regex;

        match stream.comments() {
            Some(comment) => {
                let map = comment.to_map();
                let title = map
                    .get("TITLE")
                    .map(|v| v.value().to_owned())
                    .or_else(|| {
                        // use filename as default track name
                        let reg = Regex::new(r#"^\d{2,3}(?:\s?[.-]\s?|\s)(.+)$"#).unwrap();
                        let input = stream.path.file_stem().and_then(|s| s.to_str())?;
                        let filename = reg
                            .captures(input)
                            .and_then(|c| c.get(1))
                            .map(|r| r.as_str().to_string())
                            .unwrap_or_else(|| input.to_string());
                        Some(filename)
                    })
                    .unwrap_or_default();
                // auto audio type for instrumental, drama and radio
                let track_type = TrackType::guess(&title);
                Track::new(
                    title,
                    map.get("ARTIST").map(|v| v.value().to_string()),
                    None,
                    track_type,
                    Default::default(),
                )
            }
            None => Track::empty(),
        }
    }
}
