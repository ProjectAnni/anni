use crate::common::{Backend, extract_catalog};
use async_trait::async_trait;
use tokio::io::AsyncRead;
use std::collections::HashMap;
use std::path::{PathBuf, Path};
use tokio::fs::{read_dir, File};
use thiserror::Error;

pub struct FileBackend {
    root: PathBuf,
    inner: HashMap<String, PathBuf>,
}

impl FileBackend {
    pub fn new(root: PathBuf) -> Self {
        FileBackend { root, inner: Default::default() }
    }

    async fn walk_dir<P: AsRef<Path> + Send>(&mut self, dir: P, to_visit: &mut Vec<PathBuf>) -> Result<(), FileBackendError> {
        let mut dir = read_dir(dir).await?;
        while let Some(entry) = dir.next_entry().await? {
            if entry.metadata().await?.is_dir() {
                let path = entry.path();
                if let Some(catalog) = extract_catalog(
                    path.file_name().ok_or(FileBackendError::InvalidPath)?
                        .to_str().ok_or(FileBackendError::InvalidPath)?
                ) {
                    self.inner.insert(catalog, path);
                } else {
                    to_visit.push(path);
                }
            }
        }
        Ok(())
    }
}

#[async_trait]
impl Backend for FileBackend {
    type Err = FileBackendError;

    async fn update_albums(&mut self) -> Result<Vec<&str>, Self::Err> {
        self.inner.clear();

        let mut to_visit = Vec::new();
        self.walk_dir(&self.root.clone(), &mut to_visit).await?;

        while let Some(dir) = to_visit.pop() {
            self.walk_dir(dir, &mut to_visit).await?;
        }
        Ok(self.inner.keys().map(|r| r.as_str()).collect())
    }

    async fn get_audio(&self, catalog: &str, track_id: u8, track_name: &str) -> Result<Box<dyn AsyncRead>, Self::Err> {
        if let Some(path) = self.inner.get(catalog) {
            let mut p = path.clone();
            p.push(format!("{:02}. {}.flac", track_id, track_name));
            let file = File::open(p).await?;
            Ok(Box::new(file))
        } else {
            Err(FileBackendError::UnknownCatalog)
        }
    }
}

#[derive(Debug, Error)]
pub enum FileBackendError {
    #[error("unknown catalog")]
    UnknownCatalog,
    #[error("invalid path")]
    InvalidPath,
    #[error(transparent)]
    IOError(#[from] std::io::Error),
}

#[tokio::test]
async fn test_scan() {
    let mut f = FileBackend::new(PathBuf::from("/home/yesterday17/音乐/"));
    let d = f.update_albums().await.unwrap();
    println!("{:#?}", d);

    let _audio = f.get_audio("LACM-14986", 2, "Anniversary").await.unwrap();
}