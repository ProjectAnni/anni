use crate::{Range, ResourceReader};
use anni_flac::blocks::BlockStreamInfo;
use anni_flac::prelude::{AsyncDecode, Encode, Result};
use std::io::Cursor;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};

async fn read_header<R>(mut reader: R) -> Result<(BlockStreamInfo, ResourceReader)>
where
    R: AsyncRead + Unpin + Send + 'static,
{
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

pub(crate) async fn read_duration(
    reader: ResourceReader,
    range: Range,
) -> Result<(u64, ResourceReader)> {
    if !range.contains_flac_header() {
        return Ok((0, reader));
    }

    let (info, reader) = read_header(reader).await?;
    let duration = info.total_samples * 1000 / info.sample_rate as u64;
    Ok((duration, Box::pin(reader)))
}
