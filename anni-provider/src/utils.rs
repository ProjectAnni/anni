use std::io::{Cursor};
use std::str::FromStr;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use anni_flac::blocks::BlockStreamInfo;
use anni_flac::prelude::{AsyncDecode, Encode, Result};
use crate::ResourceReader;

pub(crate) async fn read_header<R>(mut reader: R) -> Result<(BlockStreamInfo, ResourceReader)>
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

pub(crate) fn does_range_contain_flac_header(range: &Option<String>) -> bool {
    match range {
        Some(range) => {
            let range = range.split('=').nth(1).unwrap();
            let (start, end) = range.split_once('-').unwrap_or(("0", ""));
            let start = usize::from_str(start).unwrap_or(0);
            let end = usize::from_str(end).unwrap_or(usize::MAX);
            start == 0 && end >= 42
        }
        None => true,
    }
}
