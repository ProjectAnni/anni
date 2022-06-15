use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;
use async_trait::async_trait;
use anni_repo::db::RepoDatabaseRead;
use crate::{AnniProvider, AudioResourceReader, FileEntry, FileSystemProvider, ProviderError, Range, ResourceReader, Result};

pub struct CommonConventionProvider {
    root: PathBuf,
    fs: Box<dyn FileSystemProvider + Send + Sync>,
    repo: Mutex<RepoDatabaseRead>,

    albums: HashMap<String, FileEntry>,
    discs: HashMap<String, Vec<FileEntry>>,
}

#[async_trait]
impl AnniProvider for CommonConventionProvider {
    async fn albums(&self) -> Result<HashSet<Cow<str>>> {
        Ok(self.albums.keys().map(|s| Cow::Borrowed(s.as_str())).collect())
    }

    async fn get_audio(&self, album_id: &str, disc_id: u8, track_id: u8, range: Range) -> Result<AudioResourceReader> {
        let disc = self.get_disc(album_id, disc_id)?;
        let file = self.fs.get_file_entry_by_prefix(&disc.path, &format!("{:02}.", track_id)).await?;
        self.fs.get_audio_file(&file.path, range).await
    }

    async fn get_cover(&self, album_id: &str, disc_id: Option<u8>) -> Result<ResourceReader> {
        let folder = match disc_id {
            Some(disc_id) => self.get_disc(album_id, disc_id)?,
            _ => self.albums.get(album_id).ok_or(ProviderError::FileNotFound)?,
        };
        self.fs.get_file(&folder.path, Range::FULL).await
    }

    async fn reload(&mut self) -> Result<()> {
        self.fs.reload().await?;
        self.repo.lock().unwrap().reload()?;
        self.reload_albums().await?;
        Ok(())
    }
}

impl CommonConventionProvider {
    pub fn get_disc(&self, album_id: &str, disc_id: u8) -> Result<&FileEntry> {
        if !self.albums.contains_key(album_id) {
            return Err(ProviderError::FileNotFound);
        }

        let folders = self.discs.get(album_id).ok_or(ProviderError::FileNotFound)?;
        folders.get(disc_id as usize - 1).ok_or(ProviderError::FileNotFound)
    }

    pub async fn reload_albums(&mut self) -> Result<()> {
        self.albums.clear();
        self.discs.clear();

        // TODO: load albums

        Ok(())
    }
}
