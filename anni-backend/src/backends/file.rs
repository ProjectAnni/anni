use crate::common::{Backend, extract_album, extract_disc, BackendError};
use async_trait::async_trait;
use tokio::io::AsyncRead;
use std::collections::{HashMap, HashSet};
use std::path::{PathBuf, Path};
use tokio::fs::{read_dir, File};
use std::pin::Pin;

pub struct FileBackend {
    strict: bool,
    root: PathBuf,
    inner: HashMap<String, PathBuf>,
}

impl FileBackend {
    pub fn new(root: PathBuf, strict: bool) -> Self {
        FileBackend { root, strict, inner: Default::default() }
    }

    async fn walk_dir<P: AsRef<Path> + Send>(&mut self, dir: P, to_visit: &mut Vec<PathBuf>) -> Result<(), BackendError> {
        let mut dir = read_dir(dir).await?;
        while let Some(entry) = dir.next_entry().await? {
            if entry.metadata().await?.is_dir() {
                let path = entry.path();
                if let Some(catalog) = extract_album(
                    path.file_name().ok_or(BackendError::InvalidPath)?
                        .to_str().ok_or(BackendError::InvalidPath)?
                ) {
                    // look for inner discs
                    if !self.walk_discs(&path).await? {
                        // no disc found, one disc by default
                        self.inner.insert(catalog, path);
                    }
                } else {
                    to_visit.push(path);
                }
            }
        }
        Ok(())
    }

    async fn walk_discs<P: AsRef<Path> + Send>(&mut self, album: P) -> Result<bool, BackendError> {
        let mut dir = read_dir(album).await?;
        let mut has_disc = false;
        while let Some(entry) = dir.next_entry().await? {
            if entry.metadata().await?.is_dir() {
                let path = entry.path();
                let disc_name = path.file_name().ok_or(BackendError::InvalidPath)?
                    .to_str().ok_or(BackendError::InvalidPath)?;
                if let Some(catalog) = extract_disc(disc_name) {
                    self.inner.insert(catalog, path);
                    has_disc = true;
                }
            }
        }
        Ok(has_disc)
    }

    async fn update(&self) -> Result<Vec<String>, BackendError> {
        let mut albums: Vec<String> = Vec::new();
        let mut dir = read_dir(&self.root).await?;
        while let Some(entry) = dir.next_entry().await? {
            if entry.metadata().await?.is_dir() {
                let path = entry.path();
                let catalog = path.file_name()
                    .ok_or(BackendError::InvalidPath)?
                    .to_str()
                    .ok_or(BackendError::InvalidPath)?;
                albums.push(catalog.to_string());
            }
        }
        Ok(albums)
    }

    fn get_catalog_path(&self, catalog: &str) -> Result<PathBuf, BackendError> {
        Ok(if self.strict {
            self.root.join(catalog)
        } else {
            self.inner.get(catalog).ok_or(BackendError::UnknownCatalog)?.to_owned()
        })
    }
}

#[async_trait]
impl Backend for FileBackend {
    async fn albums(&mut self) -> Result<HashSet<String>, BackendError> {
        if self.strict {
            Ok(self.update().await?.into_iter().collect())
        } else {
            self.inner.clear();

            let mut to_visit = Vec::new();
            self.walk_dir(&self.root.clone(), &mut to_visit).await?;

            while let Some(dir) = to_visit.pop() {
                self.walk_dir(dir, &mut to_visit).await?;
            }
            Ok(self.inner.keys().into_iter().map(|a| a.to_owned()).collect())
        }
    }

    async fn get_audio(&self, catalog: &str, track_id: u8) -> Result<Pin<Box<dyn AsyncRead>>, BackendError> {
        let path = self.get_catalog_path(catalog)?;
        let mut dir = read_dir(path).await?;
        while let Some(entry) = dir.next_entry().await? {
            let filename = entry.file_name();
            if filename.to_string_lossy().starts_with::<&str>(format!("{:02}.", track_id).as_ref()) {
                let file = File::open(entry.path()).await?;
                let result: Pin<Box<dyn AsyncRead>> = Box::pin(file);
                return Ok(result);
            }
        }
        Err(BackendError::FileNotFound)
    }

    async fn get_cover(&self, catalog: &str) -> Result<Pin<Box<dyn AsyncRead>>, BackendError> {
        let path = self.get_catalog_path(catalog)?;
        let path = path.join("cover.jpg");
        let file = File::open(path).await?;
        Ok(Box::pin(file))
    }
}

#[tokio::test]
async fn test_scan() {
    let mut f = FileBackend::new(PathBuf::from("/home/yesterday17/音乐/"), false);
    let d = f.update_albums().await.unwrap();
    println!("{:#?}", d);

    let _audio = f.get_audio("LACM-14986", 2, "Anniversary").await.unwrap();
}