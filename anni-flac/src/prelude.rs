use std::io::{Read, Write};

pub type Result<I> = std::result::Result<I, crate::error::FlacError>;

pub trait Decode: Sized {
    fn from_reader<R: Read>(reader: &mut R) -> Result<Self>;
}

#[cfg(feature = "async")]
pub(crate) use tokio::io::{AsyncRead, AsyncReadExt};

#[cfg(feature = "async")]
#[async_trait::async_trait]
pub trait AsyncDecode: Sized {
    async fn from_async_reader<R>(reader: &mut R) -> Result<Self>
    where
        R: AsyncRead + Unpin + Send;
}

pub trait Encode: Sized {
    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()>;
}
