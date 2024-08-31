use crate::prelude::*;
use anni_metadata::model::{Album, Disc, Tag, TagString, TagType, Track, TrackType};
use std::collections::HashMap;

pub trait ResolveTags {
    fn resolve_tags(&mut self, tags: &HashMap<String, HashMap<TagType, Tag>>) -> RepoResult<()>;
}

impl ResolveTags for Album {
    fn resolve_tags(&mut self, tags: &HashMap<String, HashMap<TagType, Tag>>) -> RepoResult<()> {
        for tag in self.info.tags.iter_mut() {
            tag.resolve_tags(tags)?;
        }

        for disc in self.discs.iter_mut() {
            disc.resolve_tags(tags)?;
        }

        Ok(())
    }
}

impl ResolveTags for Disc {
    fn resolve_tags(&mut self, tags: &HashMap<String, HashMap<TagType, Tag>>) -> RepoResult<()> {
        for tag in self.tags.iter_mut() {
            tag.resolve_tags(tags)?;
        }
        for track in self.tracks.iter_mut() {
            track.resolve_tags(tags)?;
        }

        Ok(())
    }
}

impl ResolveTags for Track {
    fn resolve_tags(&mut self, tags: &HashMap<String, HashMap<TagType, Tag>>) -> RepoResult<()> {
        for tag in self.tags.iter_mut() {
            tag.resolve_tags(tags)?;
        }

        Ok(())
    }
}

impl ResolveTags for TagString {
    fn resolve_tags(&mut self, tags: &HashMap<String, HashMap<TagType, Tag>>) -> RepoResult<()> {
        if let TagType::Unknown = self.tag_type() {
            if let Some(tags) = tags.get(self.name()) {
                if tags.len() > 1 {
                    return Err(Error::RepoTagDuplicated(self.full_clone()));
                }

                let actual_type = tags.values().next().unwrap().tag_type().clone();
                self.set_tag_type(actual_type);
            }
        }

        Ok(())
    }
}

#[cfg(feature = "apply")]
pub trait ApplyMetadata {
    fn apply_strict<P>(
        &self,
        directory: P,
        detailed: bool,
    ) -> Result<(), crate::error::AlbumApplyError>
    where
        P: AsRef<std::path::Path>;

    fn apply_convention<P>(&self, directory: P) -> Result<(), crate::error::AlbumApplyError>
    where
        P: AsRef<std::path::Path>;
}

#[cfg(feature = "apply")]
impl ApplyMetadata for Album {
    /// Apply album metadata to a directory formatted with strict album format.
    ///
    /// This function applies both metadata and cover.
    ///
    /// The argument `detailed` determines whether to write metadata(such as title and artist) and cover to flac files.
    fn apply_strict<P>(
        &self,
        directory: P,
        detailed: bool,
    ) -> Result<(), crate::error::AlbumApplyError>
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
                // TODO: the comparison is not accurate if `detailed` = false
                if comments.is_none() || comments.unwrap().to_string() != meta {
                    let comments = flac.comments_mut();
                    comments.clear();

                    if detailed {
                        comments.push(UserComment::title(track.title()));
                        comments.push(UserComment::album(disc.title()));
                        comments.push(UserComment::artist(track.artist()));
                        comments.push(UserComment::date(self.release_date()));
                    }
                    comments.push(UserComment::track_number(track_num));
                    comments.push(UserComment::track_total(track_total));
                    comments.push(UserComment::disc_number(disc_num));
                    comments.push(UserComment::disc_total(disc_total));
                    // modified = true;
                }

                if detailed {
                    // TODO: do not modify flac file if embed cover is the same as the one in folder
                    let cover_path = file.with_file_name("cover.jpg");
                    let picture =
                        BlockPicture::new(cover_path, PictureType::CoverFront, String::new())?;
                    flac.blocks
                        .retain(|block| !matches!(block.data, MetadataBlockData::Picture(_)));
                    flac.blocks
                        .push(MetadataBlock::new(MetadataBlockData::Picture(picture)));
                    // modified = true;
                } else {
                    // remove cover block
                    flac.blocks
                        .retain(|block| !matches!(block.data, MetadataBlockData::Picture(_)));
                }

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
    fn apply_convention<P>(&self, directory: P) -> Result<(), crate::error::AlbumApplyError>
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

pub struct RepoTrack(pub Track);

#[cfg(feature = "flac")]
impl From<anni_flac::FlacHeader> for RepoTrack {
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
                RepoTrack(Track::new(
                    title,
                    map.get("ARTIST").map(|v| v.value().to_string()),
                    None,
                    track_type,
                    Default::default(),
                ))
            }
            None => RepoTrack(Track::empty()),
        }
    }
}
