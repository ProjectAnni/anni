use std::future::Future;
use std::io::SeekFrom;
use std::path::PathBuf;
use std::pin::Pin;
use std::task::Poll;
use async_trait::async_trait;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_stream::Stream;
use crate::{FileEntry, FileSystemProvider, Range, ResourceReader};

struct FutureWrapper<T: Clone>(T);

impl<T: Clone> Future for FutureWrapper<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, _: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        Poll::Ready(self.0.clone())
    }
}

pub struct LocalFileSystemProvider {}

#[async_trait]
impl FileSystemProvider for LocalFileSystemProvider {
    async fn children(&self, path: &PathBuf) -> crate::Result<Pin<Box<dyn Stream<Item=FileEntry> + Send>>> {
        todo!()
        // Ok(Box::new(
        //     std::fs::read_dir(path)?
        //         .filter_map(|entry| {
        //             let entry = entry.unwrap();
        //             let path = entry.path();
        //             let name = path.file_name().unwrap().to_string_lossy().to_string();
        //             if path.is_dir() {
        //                 Some(wrap(FileEntry { name, path }))
        //             } else {
        //                 None
        //             }
        //         })
        // ))
    }

    async fn get_file_entry_by_prefix(&self, parent: &PathBuf, prefix: &str) -> crate::Result<FileEntry> {
        todo!()
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