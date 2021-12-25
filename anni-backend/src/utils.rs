use std::io::{Cursor};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use anni_flac::blocks::BlockStreamInfo;
use anni_flac::prelude::{AsyncDecode, Encode, Result};
use crate::BackendReader;

pub(crate) async fn read_header<R>(mut reader: R) -> Result<(BlockStreamInfo, BackendReader)>
    where R: AsyncRead + Unpin + Send + 'static {
    let first = reader.read_u32().await.unwrap();
    let second = reader.read_u32().await.unwrap();
    let info = BlockStreamInfo::from_async_reader(&mut reader).await?;

    let mut header = Cursor::new(Vec::with_capacity(4 + 4 + 34));
    header.write_u32(first).await.unwrap();
    header.write_u32(second).await.unwrap();
    info.write_to(&mut header).unwrap();
    header.set_position(0);

    Ok((info, Box::pin(header.chain(reader))))
}
