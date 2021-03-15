use std::path::PathBuf;
use crate::{Backend, BackendError};
use std::pin::Pin;
use tokio::io::AsyncRead;
use tokio::fs::{read_dir, File};
use async_trait::async_trait;
use std::borrow::Cow;

pub struct StrictFileBackend {
    root: PathBuf,
}

impl StrictFileBackend {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub async fn update(&self) -> Result<Vec<String>, BackendError> {
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
}

#[async_trait]
impl Backend for StrictFileBackend {
    fn need_cache(&self) -> bool {
        false
    }

    async fn has(&self, catalog: &str) -> bool {
        self.update().await
            .map(|l| l.iter().any(|s| s == catalog))
            .unwrap_or(false)
    }

    async fn albums(&self) -> Vec<Cow<str>> {
        self.update().await.unwrap_or(Vec::new()).into_iter().map(|a| Cow::Owned(a)).collect()
    }

    async fn update_albums(&mut self) -> Result<(), BackendError> {
        // self.update()?;
        Ok(())
    }

    async fn get_audio(&self, catalog: &str, track_id: u8) -> Result<Pin<Box<dyn AsyncRead>>, BackendError> {
        let path = self.root.join(catalog);
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
        let path = self.root.join(catalog).join("cover.jpg");
        let file = File::open(path).await?;
        Ok(Box::pin(file))
    }
}
