use tokio::io::AsyncRead;
use anni_flac::blocks::BlockStreamInfo;
use anni_flac::prelude::*;
use tokio::io::AsyncReadExt;

pub(crate) async fn read_header<R>(mut reader: &mut R) -> Result<(u32, u32, BlockStreamInfo)>
    where R: AsyncRead + Unpin + Send {
    let first = reader.read_u32().await.unwrap();
    let second = reader.read_u32().await.unwrap();
    let info = BlockStreamInfo::from_async_reader(&mut reader).await?;
    Ok((first, second, info))
}