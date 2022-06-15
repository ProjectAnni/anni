use std::io::SeekFrom;
use std::path::PathBuf;
use std::pin::Pin;
use async_trait::async_trait;
use tokio::fs::read_dir;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_stream::{Stream, self as stream};
use crate::{FileEntry, FileSystemProvider, ProviderError, Range, ResourceReader};

pub struct LocalFileSystemProvider;

#[async_trait]
impl FileSystemProvider for LocalFileSystemProvider {
    async fn children(&self, path: &PathBuf) -> crate::Result<Pin<Box<dyn Stream<Item=FileEntry> + Send>>> {
        Ok(Box::pin(stream::iter(
            std::fs::read_dir(path)?
                .filter_map(|entry| {
                    let entry = entry.ok()?;
                    let path = entry.path();
                    let name = path.file_name()?.to_string_lossy().to_string();
                    if path.is_dir() {
                        Some(FileEntry { name, path })
                    } else {
                        None
                    }
                })
        )))
    }

    async fn get_file_entry_by_prefix(&self, parent: &PathBuf, prefix: &str) -> crate::Result<FileEntry> {
        let mut dir = read_dir(parent).await?;
        loop {
            match dir.next_entry().await? {
                Some(entry)
                if entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(prefix) =>
                    {
                        return Ok(FileEntry {
                            name: entry.file_name().to_string_lossy().to_string(),
                            path: entry.path(),
                        });
                    }
                None => return Err(ProviderError::FileNotFound),
                _ => {}
            }
        };
    }

    async fn get_file(&self, path: &PathBuf, range: Range) -> crate::Result<ResourceReader> {
        let mut file = tokio::fs::File::open(path).await?;
        let metadata = file.metadata().await?;
        let file_size = metadata.len();

        file.seek(SeekFrom::Start(range.start)).await?;
        let file = file.take(range.length_limit(file_size));
        Ok(Box::pin(file))
    }

    async fn get_audio_info(&self, path: &PathBuf) -> crate::Result<(String, usize)> {
        let extension = path.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_default();
        let size = tokio::fs::metadata(path).await.map(|m| m.len())?;
        Ok((extension, size as usize))
    }

    async fn reload(&mut self) -> crate::Result<()> {
        Ok(())
    }
}