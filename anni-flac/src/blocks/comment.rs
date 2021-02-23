use crate::{Result, Decode};
use std::io::Read;
use byteorder::{ReadBytesExt, LittleEndian};
use crate::common::take_string;

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
#[derive(Debug)]
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
    pub fn insert(&mut self, comment: UserComment) {
        self.comments.push(comment);
    }

    pub fn len(&self) -> usize {
        self.comments.len()
    }
}

impl ToString for BlockVorbisComment {
    fn to_string(&self) -> String {
        let mut result = String::new();
        for comment in self.comments.iter() {
            if !result.is_empty() {
                result += "\n";
            }
            result += &format!("{}={}", comment.key_raw(), comment.value());
        }
        result
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

    pub fn entry(&self) -> String {
        self.comment.clone()
    }
}

impl Decode for UserComment {
    fn from_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let length = reader.read_u16::<LittleEndian>()?;
        let comment = take_string(reader, length as usize)?;
        Ok(UserComment::new(comment))
    }
}

