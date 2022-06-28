use crate::prelude::*;
use crate::utils::*;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::collections::HashMap;
use std::fmt;
use std::fmt::Display;
use std::io::{Read, Write};

/// Also known as FLAC tags, the contents of a vorbis comment packet as specified here (without the framing bit).
/// Note that the vorbis comment spec allows for on the order of 2 ^ 64 bytes of data where as the FLAC metadata block is limited to 2 ^ 24 bytes.
/// Given the stated purpose of vorbis comments, i.e. human-readable textual information, this limit is unlikely to be restrictive.
/// Also note that the 32-bit field lengths are **little-endian** coded according to the vorbis spec, as opposed to the usual big-endian coding of fixed-length integers in the rest of FLAC.
///
/// The Vorbis text comment header is the second (of three) header packets that begin a Vorbis bitstream.
/// It is meant for short, text comments, not arbitrary metadata; arbitrary metadata belongs in a separate logical bitstream (usually an XML stream type) that provides greater structure and machine parseability.
///
/// The comment field is meant to be used much like someone jotting a quick note on the bottom of a CDR.
/// It should be a little information to remember the disc by and explain it to others; a short, to-the-point text note that need not only be a couple words, but isn't going to be more than a short paragraph.
///
/// The essentials, in other words, whatever they turn out to be, eg:
///     "Honest Bob and the Factory-to-Dealer-Incentives, _I'm Still Around_, opening for Moxy Früvous, 1997"
pub struct BlockVorbisComment {
    // [vendor_length] = read an unsigned integer of 32 bits
    // vendor_length: u32,
    /// [vendor_string] = read a UTF-8 vector as [vendor_length] octets
    pub vendor_string: String,

    // [user_comment_list_length] = read an unsigned integer of 32 bits
    // comment_number: u32,
    /// iterate [user_comment_list_length] times
    pub comments: Vec<UserComment>,
    // [framing_bit] = read a single bit as boolean
    // if ( [framing_bit] unset or end of packet ) then ERROR
}

impl BlockVorbisComment {
    pub fn push(&mut self, comment: UserComment) {
        self.comments.push(comment);
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.comments.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clear(&mut self) {
        self.comments.clear()
    }

    pub fn to_map(&self) -> HashMap<String, &UserComment> {
        let mut map: HashMap<_, _> = Default::default();
        for comment in self.comments.iter() {
            // NOT override only when key exists AND comment.value is EMPTY.
            if !(map.contains_key(&comment.key()) && comment.value().is_empty()) {
                map.insert(comment.key(), comment);
            }
        }
        map
    }
}

impl Decode for BlockVorbisComment {
    fn from_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let vendor_length = reader.read_u32::<LittleEndian>()?;
        let vendor_string = take_string(reader, vendor_length as usize)?;
        let comment_number = reader.read_u32::<LittleEndian>()?;
        let mut comments = Vec::with_capacity(comment_number as usize);

        for _ in 0..comment_number {
            comments.push(UserComment::from_reader(reader)?);
        }

        Ok(BlockVorbisComment {
            vendor_string,
            comments,
        })
    }
}

#[cfg(feature = "async")]
#[async_trait::async_trait]
impl AsyncDecode for BlockVorbisComment {
    async fn from_async_reader<R>(reader: &mut R) -> Result<Self>
    where
        R: AsyncRead + Unpin + Send,
    {
        let vendor_length = reader.read_u32_le().await?;
        let vendor_string = take_string_async(reader, vendor_length as usize).await?;
        let comment_number = reader.read_u32_le().await?;
        let mut comments = Vec::with_capacity(comment_number as usize);

        for _ in 0..comment_number {
            comments.push(UserComment::from_async_reader(reader).await?);
        }

        Ok(BlockVorbisComment {
            vendor_string,
            comments,
        })
    }
}

impl Encode for BlockVorbisComment {
    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_u32::<LittleEndian>(self.vendor_string.len() as u32)?;
        writer.write_all(self.vendor_string.as_bytes())?;
        writer.write_u32::<LittleEndian>(self.comments.len() as u32)?;
        for comment in self.comments.iter() {
            comment.write_to(writer)?;
        }
        Ok(())
    }
}

impl fmt::Debug for BlockVorbisComment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut prefix = "".to_owned();
        if let Some(width) = f.width() {
            prefix = " ".repeat(width);
        }
        writeln!(
            f,
            "{prefix}vendor string: {}",
            self.vendor_string,
            prefix = prefix
        )?;
        writeln!(f, "{prefix}comments: {}", self.len(), prefix = prefix)?;
        for (i, c) in self.comments.iter().enumerate() {
            write!(f, "{}", prefix)?;
            writeln!(
                f,
                "{prefix}comment[{}]: {}={}",
                i,
                c.key_raw(),
                c.value(),
                prefix = prefix
            )?;
        }
        Ok(())
    }
}

impl fmt::Display for BlockVorbisComment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for c in self.comments.iter() {
            writeln!(f, "{}", c.comment)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct UserComment {
    // [length] = read an unsigned integer of 32 bits
    // length: u32,
    /// this iteration's user comment = read a UTF-8 vector as [length] octets
    comment: String,
    value_offset: Option<usize>,
}

impl UserComment {
    pub fn new(comment: String) -> Self {
        let value_offset = comment.find('=');
        Self {
            comment,
            value_offset,
        }
    }

    pub fn key(&self) -> String {
        self.key_raw().to_ascii_uppercase()
    }

    pub fn key_raw(&self) -> &str {
        match self.value_offset {
            Some(offset) => &self.comment[..offset],
            None => &self.comment,
        }
    }

    pub fn is_key_uppercase(&self) -> bool {
        let key = match self.value_offset {
            Some(offset) => &self.comment[..offset],
            None => &self.comment,
        };

        key.chars().all(|c| !c.is_ascii_lowercase())
    }

    pub fn value(&self) -> &str {
        match self.value_offset {
            Some(offset) => &self.comment[offset + 1..],
            None => &self.comment[self.comment.len()..],
        }
    }

    pub fn len(&self) -> usize {
        self.comment.len()
    }

    pub fn entry(&self) -> String {
        self.comment.clone()
    }

    pub fn is_empty(&self) -> bool {
        self.comment.is_empty()
    }

    pub fn clear(&mut self) {
        self.comment.clear();
        self.value_offset = None;
    }
}

impl Decode for UserComment {
    fn from_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let length = reader.read_u32::<LittleEndian>()?;
        let comment = take_string(reader, length as usize)?;
        Ok(UserComment::new(comment))
    }
}

#[cfg(feature = "async")]
#[async_trait::async_trait]
impl AsyncDecode for UserComment {
    async fn from_async_reader<R>(reader: &mut R) -> Result<Self>
    where
        R: AsyncRead + Unpin + Send,
    {
        let length = reader.read_u32_le().await?;
        let comment = take_string_async(reader, length as usize).await?;
        Ok(UserComment::new(comment))
    }
}

impl Encode for UserComment {
    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_u32::<LittleEndian>(self.comment.len() as u32)?;
        writer.write_all(self.comment.as_bytes())?;
        Ok(())
    }
}

pub trait UserCommentExt {
    fn title<S>(value: S) -> Self
    where
        S: Display;
    fn artist<S>(value: S) -> Self
    where
        S: Display;
    fn album<S>(value: S) -> Self
    where
        S: Display;
    fn date<S>(value: S) -> Self
    where
        S: Display;
    fn track_number<S>(value: S) -> Self
    where
        S: Display;
    fn track_total<S>(value: S) -> Self
    where
        S: Display;
    fn disc_number<S>(value: S) -> Self
    where
        S: Display;
    fn disc_total<S>(value: S) -> Self
    where
        S: Display;
    fn album_artist<S>(value: S) -> Self
    where
        S: Display;
}

impl UserCommentExt for UserComment {
    fn title<S>(value: S) -> Self
    where
        S: Display,
    {
        Self::new(format!("TITLE={}", value))
    }

    fn artist<S>(value: S) -> Self
    where
        S: Display,
    {
        Self::new(format!("ARTIST={}", value))
    }

    fn album<S>(value: S) -> Self
    where
        S: Display,
    {
        Self::new(format!("ALBUM={}", value))
    }

    fn date<S>(value: S) -> Self
    where
        S: Display,
    {
        Self::new(format!("DATE={}", value))
    }

    fn track_number<S>(value: S) -> Self
    where
        S: Display,
    {
        Self::new(format!("TRACKNUMBER={}", value))
    }

    fn track_total<S>(value: S) -> Self
    where
        S: Display,
    {
        Self::new(format!("TRACKTOTAL={}", value))
    }

    fn disc_number<S>(value: S) -> Self
    where
        S: Display,
    {
        Self::new(format!("DISCNUMBER={}", value))
    }

    fn disc_total<S>(value: S) -> Self
    where
        S: Display,
    {
        Self::new(format!("DISCTOTAL={}", value))
    }

    fn album_artist<S>(value: S) -> Self
    where
        S: Display,
    {
        Self::new(format!("ALBUMARTIST={}", value))
    }
}
