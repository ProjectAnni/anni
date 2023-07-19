use crate::{
    strict_album_path, AnniProvider, AudioInfo, AudioResourceReader, Range, ResourceReader,
};
use std::borrow::Cow;
use std::collections::{HashSet, VecDeque};
use std::io::SeekFrom;
use std::num::NonZeroU8;
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use uuid::Uuid;

/// `NoCacheStrictLocalProvider` defines a providers which serves local files without caching,
/// which is useful for development and testing.
///
/// The main purpose of using this provider is to support annil for anni workspace.
pub struct NoCacheStrictLocalProvider {
    /// Root directory of an strict annil
    pub root: PathBuf,
    /// Hash layers
    pub layer: usize,
}

#[async_trait::async_trait]
impl AnniProvider for NoCacheStrictLocalProvider {
    /// During the list operation, the provider will scan the whole library and return all available albums.
    ///
    /// Here **available** means the album directory exists and is not empty.
    /// The non-empty assumption is made because a workspace may create an empty directory for further use.
    async fn albums(&self) -> crate::Result<HashSet<Cow<str>>> {
        // pre-allocate with capacity = 32
        // assume commonly there are around 32 albums in a workspace
        let mut albums = HashSet::with_capacity(32);

        // scan the root directory
        let mut vis = VecDeque::from([(self.root.clone(), 0)]);
        while let Some((ref path, layer)) = vis.pop_front() {
            log::debug!("Walking dir: {path:?}");
            let mut reader = path.read_dir()?;
            if layer == self.layer {
                while let Some(entry) = reader.next() {
                    let entry = entry?;
                    match Uuid::parse_str(&entry.file_name().to_string_lossy()) {
                        Ok(album_id) => {
                            albums.insert(album_id.to_string());
                        }
                        _ => log::warn!("Unexpected dir: {path:?}"),
                    }
                }
            } else {
                while let Some(entry) = reader.next() {
                    let entry = entry?;
                    if entry.metadata()?.is_dir() {
                        vis.push_back((entry.path(), layer + 1));
                    }
                }
            }
        }

        Ok(albums.into_iter().map(Cow::Owned).collect())
    }

    async fn get_audio(
        &self,
        album_id: &str,
        disc_id: NonZeroU8,
        track_id: NonZeroU8,
        range: Range,
    ) -> crate::Result<AudioResourceReader> {
        let mut audio = strict_album_path(&self.root, album_id, self.layer);
        audio.push(disc_id.get().to_string());
        audio.push(format!("{track_id}.flac"));

        if !audio.exists() {
            return Err(crate::ProviderError::FileNotFound);
        }

        let mut file = tokio::fs::File::open(audio).await?;
        let metadata = file.metadata().await?;
        let file_size = metadata.len();

        file.seek(SeekFrom::Start(range.start)).await?;
        let file = file.take(range.length_limit(file_size));
        let reader = Box::pin(file);
        let (duration, reader) = crate::utils::read_duration(reader, range).await?;

        Ok(AudioResourceReader {
            info: AudioInfo {
                extension: "flac".to_string(),
                size: file_size as usize,
                duration,
            },
            range: Range {
                start: range.start,
                end: Some(range.end.unwrap_or(file_size - 1)),
                total: Some(file_size),
            },
            reader,
        })
    }

    async fn get_cover(
        &self,
        album_id: &str,
        disc_id: Option<NonZeroU8>,
    ) -> crate::Result<ResourceReader> {
        let mut cover = strict_album_path(&self.root, album_id, self.layer);
        if let Some(disc_id) = disc_id {
            cover.push(disc_id.get().to_string());
        }
        cover.push(format!("cover.jpg"));

        if !cover.exists() {
            return Err(crate::ProviderError::FileNotFound);
        }

        let file = tokio::fs::File::open(cover).await?;
        Ok(Box::pin(file))
    }

    async fn reload(&mut self) -> crate::Result<()> {
        Ok(())
    }
}
