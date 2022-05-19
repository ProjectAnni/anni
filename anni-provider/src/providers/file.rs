use std::borrow::Cow;
use crate::common::{AnniProvider, AudioResourceReader, ProviderError};
use anni_repo::library::{album_info, disc_info};
use async_trait::async_trait;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::SeekFrom;
use std::path::{Path, PathBuf};
use parking_lot::Mutex;
use tokio::fs::{read_dir, File};
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use anni_repo::db::RepoDatabaseRead;
use crate::{AudioInfo, Range, ResourceReader};
use uuid::Uuid;

pub struct FileBackend {
    root: PathBuf,
    repo: Mutex<RepoDatabaseRead>,
    album_path: HashMap<String, PathBuf>,
    album_discs: HashMap<String, Vec<PathBuf>>,
    strict: (bool, usize), // (enable, layer)
}

impl FileBackend {
    pub async fn new(
        root: PathBuf,
        repo: RepoDatabaseRead,
        strict: (bool, usize),
    ) -> Result<Self, ProviderError> {
        let mut this = Self {
            root,
            repo: Mutex::new(repo),
            album_path: Default::default(),
            album_discs: Default::default(),
            strict,
        };

        this.reload().await?;
        Ok(this)
    }

    async fn walk_dir<P: AsRef<Path> + Send>(
        &mut self,
        dir: P,
    ) -> Result<(), ProviderError> {
        if self.strict.0 {
            self.walk_dir_strict(dir).await
        } else {
            let mut to_visit = vec![dir.as_ref().to_owned()];
    
            while let Some(dir) = to_visit.pop() {
                self.walk_dir_instrict(dir, &mut to_visit).await?;
            }
            Ok(())    
        }
    }

    async fn walk_dir_instrict<P: AsRef<Path> + Send>(
        &mut self,
        dir: P,
        to_visit: &mut Vec<PathBuf>,
    ) -> Result<(), ProviderError> {
        log::debug!("Walking dir: {:?}", dir.as_ref());
        let mut dir = read_dir(dir).await?;
        while let Some(entry) = dir.next_entry().await? {
            if entry.metadata().await?.is_dir() {
                let path = entry.path();
                if let Ok((release_date, catalog, title, disc_count)) = album_info(
                    path.file_name()
                        .ok_or(ProviderError::InvalidPath)?
                        .to_str()
                        .ok_or(ProviderError::InvalidPath)?,
                ) {
                    log::debug!("Found album {} at: {:?}", catalog, path);
                    let album_id = self.repo.lock().match_album(&catalog, &release_date, disc_count as u8, &title)?;
                    match album_id {
                        Some(album_id) => {
                            if disc_count > 1 {
                                // look for inner discs
                                let discs = self.walk_discs(&path, disc_count).await?;
                                self.album_discs.insert(album_id.to_string(), discs);
                            }
                            self.album_path.insert(album_id.to_string(), path);
                        }
                        None => {
                            log::warn!("Album ID not found for {}, ignoring...", catalog);
                        }
                    }
                } else {
                    to_visit.push(path);
                }
            }
        }
        Ok(())
    }

    async fn walk_dir_strict<P: AsRef<Path> + Send>(
        &mut self,
        dir: P,
    ) -> Result<(), ProviderError> {
        self.walk_dir_bfs(dir.as_ref(), self.strict.1).await
    }

    async fn walk_dir_bfs(&mut self, dir: &Path, target: usize) -> Result<(), ProviderError> {
        let mut vis = VecDeque::from([(dir.to_owned(), 0)]);
        while let Some((ref path, layer)) = vis.pop_front() {
            log::debug!("Walking dir: {path:?}");
            let mut reader = read_dir(path).await?;
            if layer == target {
                while let Some(entry) = reader.next_entry().await? {
                    let path = entry.path();
                    if entry.metadata().await?.is_dir() {
                        match Uuid::parse_str(
                            entry
                                .file_name()
                                .to_str()
                                .ok_or(ProviderError::InvalidPath)?,
                        ) {
                            Ok(album_id) => {
                                self.album_path.insert(album_id.to_string(), path);
                            }
                            _ => log::warn!("Unexpected dir: {path:?}"),
                        }
                    } else {
                        log::warn!("Unexpected file: {:?}", entry.path());
                    }
                }
            } else {
                while let Some(entry) = reader.next_entry().await? {
                    if entry.metadata().await?.is_dir() {
                        vis.push_back((entry.path(), layer + 1));
                    } else {
                        log::warn!("Unexpected file: {:?}", entry.path());
                    }
                }
            }
        }
        Ok(())
    }

    async fn walk_discs<P: AsRef<Path> + Send>(
        &mut self,
        album: P,
        size: usize,
    ) -> Result<Vec<PathBuf>, ProviderError> {
        let mut discs = vec![PathBuf::new(); size];
        let mut dir = read_dir(album).await?;
        while let Some(entry) = dir.next_entry().await? {
            if entry.metadata().await?.is_dir() {
                let path = entry.path();
                let disc_name = path
                    .file_name()
                    .ok_or(ProviderError::InvalidPath)?
                    .to_str()
                    .ok_or(ProviderError::InvalidPath)?;
                if let Ok((catalog, _, disc_id)) = disc_info(disc_name) {
                    log::debug!("Found disc {} at: {:?}", catalog, path);
                    discs[disc_id - 1] = path;
                }
            }
        }
        Ok(discs)
    }

    fn get_disc(&self, album_id: &str, disc_id: u8) -> Result<&PathBuf, ProviderError> {
        log::debug!("getting disc: {album_id}/{disc_id}");
        if self.album_discs.contains_key(album_id) {
            // has multiple discs
            Ok(&self.album_discs[album_id][(disc_id - 1) as usize])
        } else if self.album_path.contains_key(album_id) {
            // has only one disc
            Ok(&self.album_path[album_id])
        } else {
            Err(ProviderError::FileNotFound)
        }
    }

    fn get_album_path(&self, album_id: &str) -> Result<&PathBuf, ProviderError> {
        Ok(self.album_path
            .get(album_id)
            .ok_or(ProviderError::FileNotFound)?)
    }
}

#[async_trait]
impl AnniProvider for FileBackend {
    async fn albums(&self) -> Result<HashSet<Cow<str>>, ProviderError> {
        Ok(self.album_path.keys().map(|s| Cow::Borrowed(s.as_str())).collect())
    }

    async fn get_audio_info(&self, album_id: &str, disc_id: u8, track_id: u8) -> Result<AudioInfo, ProviderError> {
        Ok(self.get_audio(album_id, disc_id, track_id, Range::FLAC_HEADER).await?.info)
    }

    async fn get_audio(
        &self,
        album_id: &str,
        disc_id: u8,
        track_id: u8,
        range: Range,
    ) -> Result<AudioResourceReader, ProviderError> {
        let path = self.get_disc(album_id, disc_id)?;
        let mut file = if self.strict.0 {
            File::open(
                path.join(disc_id.to_string())
                    .join(format!("{track_id}.flac")),
            )
        } else {
            let mut dir = read_dir(path).await?;
            loop {
                match dir.next_entry().await? {
                    Some(entry)
                        if entry
                            .file_name()
                            .to_string_lossy()
                            .starts_with::<&str>(format!("{:02}.", track_id).as_ref()) =>
                    {
                        break File::open(entry.path())
                    }
                    None => return Err(ProviderError::FileNotFound),
                    _ => {}
                }
            }
        }
        .await?;
        let metadata = file.metadata().await?;
        let file_size = metadata.len();

        // limit in range
        file.seek(SeekFrom::Start(range.start)).await?;
        let file = file.take(range.length_limit(file_size));

        // calculate audio duration only if it flac header is in range
        let (duration, reader): (u64, ResourceReader) = if range.contains_flac_header() {
            let (info, reader) = crate::utils::read_header(file).await?;
            (info.total_samples / info.sample_rate as u64, reader)
        } else {
            (0, Box::pin(file))
        };

        return Ok(AudioResourceReader {
            info: AudioInfo {
                extension: path.extension().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(),
                size: file_size as usize,
                duration,
            },
            range: if range.is_full() {
                range
            } else {
                range.end_with(file_size)
            },
            reader,
        });

    }

    async fn get_cover(&self, album_id: &str, disc_id: Option<u8>) -> Result<ResourceReader, ProviderError> {
        let path = match disc_id {
            Some(disc_id) if !self.strict.0 => self.get_disc(album_id, disc_id)?,
            _ => self.get_album_path(album_id)?,
            
        };
        let path = path.join("cover.jpg");
        let file = File::open(path).await?;
        Ok(Box::pin(file))
    }

    async fn reload(&mut self) -> Result<(), ProviderError> {
        self.album_discs.clear();
        self.album_path.clear();
        self.repo.lock().reload()?;

        self.walk_dir(self.root.clone()).await?;
        Ok(())
    }
}

#[cfg(feature = "test")]
mod test {
    #[tokio::test]
    async fn test_scan() {
        let mut f = FileBackend::new(PathBuf::from("/data/Music/"), (false, 0));
        let _ = f.albums().await.unwrap();
        let _audio = f.get_audio("LACM-14986", 2).await.unwrap();
    }
}
