use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use parking_lot::Mutex;
use async_trait::async_trait;
use tokio_stream::StreamExt;
use anni_repo::db::RepoDatabaseRead;
use anni_repo::library::{album_info, disc_info};
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
        self.repo.lock().reload()?;
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

        let mut to_visit = vec![self.root.clone()];
        while let Some(dir) = to_visit.pop() {
            self.walk_dir_impl(dir, &mut to_visit).await?;
        }

        Ok(())
    }

    async fn walk_dir_impl(&mut self, dir: PathBuf, to_visit: &mut Vec<PathBuf>) -> Result<()> {
        log::debug!("Walking dir: {}", dir.display());
        let mut dir = self.fs.children(&dir).await?;
        while let Some(entry) = dir.next().await {
            if let Ok((release_date, catalog, title, disc_count)) = album_info(&entry.name) {
                log::debug!("Found album {} at: {:?}", catalog, entry.path);
                let album_id = self.repo.lock().match_album(&catalog, &release_date, disc_count as u8, &title)?;
                match album_id {
                    Some(album_id) => {
                        if disc_count > 1 {
                            // look for inner discs
                            let discs = self.walk_discs(&entry.path, disc_count).await?;
                            self.discs.insert(album_id.to_string(), discs);
                        }
                        self.albums.insert(album_id.to_string(), entry);
                    }
                    None => {
                        log::warn!("Album ID not found for {}, ignoring...", catalog);
                    }
                }
            } else {
                to_visit.push(entry.path.clone());
            }
        }
        Ok(())
    }

    async fn walk_discs(&self, album: &PathBuf, size: usize) -> Result<Vec<FileEntry>> {
        let mut discs = Vec::new();
        let mut dir = self.fs.children(album).await?;
        while let Some(entry) = dir.next().await {
            if let Ok((catalog, _, disc_id)) = disc_info(&entry.name) {
                log::debug!("Found disc {} at: {:?}", catalog, entry.path);
                if disc_id <= size {
                    discs.push((disc_id, entry));
                }
            }
        }
        discs.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(discs.into_iter().map(|(_, entry)| entry).collect())
    }
}
