use crate::{
    AnniProvider, AudioResourceReader, FileEntry, FileSystemProvider, ProviderError, Range,
    ResourceReader, Result,
};
use async_trait::async_trait;
use futures::StreamExt;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet, VecDeque};
use std::num::NonZeroU8;
use std::path::PathBuf;
use uuid::Uuid;

pub struct CommonStrictProvider {
    root: PathBuf,
    layer: usize,
    fs: Box<dyn FileSystemProvider + Send + Sync>,
    folders: HashMap<String, FileEntry>,
}

impl CommonStrictProvider {
    pub async fn new(
        root: PathBuf,
        layer: usize,
        fs: Box<dyn FileSystemProvider + Send + Sync>,
    ) -> Result<Self> {
        let mut me = Self {
            root,
            layer,
            fs,
            folders: HashMap::new(),
        };
        me.reload().await?;
        Ok(me)
    }
}

#[async_trait]
impl AnniProvider for CommonStrictProvider {
    async fn albums(&self) -> Result<HashSet<Cow<str>>> {
        Ok(self
            .folders
            .keys()
            .map(|c| Cow::Borrowed(c.as_str()))
            .collect())
    }

    async fn get_audio(
        &self,
        album_id: &str,
        disc_id: NonZeroU8,
        track_id: NonZeroU8,
        range: Range,
    ) -> Result<AudioResourceReader> {
        let disc = self.get_disc(album_id, disc_id).await?;
        let file = self
            .fs
            .get_file_entry_by_prefix(&disc.path, &format!("{track_id}."))
            .await?;
        self.fs.get_audio_file(&file.path, range).await
    }

    async fn get_cover(
        &self,
        album_id: &str,
        disc_id: Option<NonZeroU8>,
    ) -> Result<ResourceReader> {
        match disc_id {
            Some(disc_id) => {
                let disc = self.get_disc(album_id, disc_id).await?;
                let cover = self
                    .fs
                    .get_file_entry_by_prefix(&disc.path, "cover.jpg")
                    .await?;
                self.fs.get_file(&cover.path, Range::FULL).await
            }
            None => {
                let album = self
                    .folders
                    .get(album_id)
                    .ok_or(ProviderError::FileNotFound)?;
                let cover = self
                    .fs
                    .get_file_entry_by_prefix(&album.path, "cover.jpg")
                    .await?;
                self.fs.get_file(&cover.path, Range::FULL).await
            }
        }
    }

    async fn reload(&mut self) -> Result<()> {
        self.fs.reload().await?;
        self.reload_albums().await?;
        Ok(())
    }
}

impl CommonStrictProvider {
    pub async fn get_disc(&self, album_id: &str, disc_id: NonZeroU8) -> Result<FileEntry> {
        let folder = self
            .folders
            .get(album_id)
            .ok_or(ProviderError::FileNotFound)?;
        let mut folders = self.fs.children(&folder.path).await?;
        while let Some(folder) = folders.next().await {
            if folder.name == format!("{disc_id}") {
                return Ok(folder);
            }
        }
        Err(ProviderError::FileNotFound)
    }

    pub async fn reload_albums(&mut self) -> Result<()> {
        self.folders.clear();

        let mut vis = VecDeque::from([(self.root.clone(), 0)]);
        while let Some((ref path, layer)) = vis.pop_front() {
            log::debug!("Walking dir: {path:?}");
            let mut reader = self.fs.children(path).await?;
            if layer == self.layer {
                while let Some(entry) = reader.next().await {
                    match Uuid::parse_str(&entry.name) {
                        Ok(album_id) => {
                            self.folders.insert(album_id.to_string(), entry);
                        }
                        _ => log::warn!("Unexpected dir: {path:?}"),
                    }
                }
            } else {
                while let Some(entry) = reader.next().await {
                    vis.push_back((entry.path, layer + 1));
                }
            }
        }

        Ok(())
    }
}
