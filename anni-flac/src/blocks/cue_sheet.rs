use crate::prelude::*;
use crate::utils::*;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::fmt;
use std::io::{Read, Write};

pub struct BlockCueSheet {
    /// <128*8> Media catalog number, in ASCII printable characters 0x20-0x7e.
    /// In general, the media catalog number may be 0 to 128 bytes long; any unused characters should be right-padded with NUL characters.
    /// For CD-DA, this is a thirteen digit number, followed by 115 NUL bytes.
    pub catalog: String,
    /// <64> The number of lead-in samples.
    /// This field has meaning only for CD-DA cuesheets; for other uses it should be 0.
    /// For CD-DA, the lead-in is the TRACK 00 area where the table of contents is stored;
    /// more precisely, it is the number of samples from the first sample of the media to the first sample of the first index point of the first track.
    /// According to the Red Book, the lead-in must be silence and CD grabbing software does not usually store it;
    /// additionally, the lead-in must be at least two seconds but may be longer.
    /// For these reasons the lead-in length is stored here so that the absolute position of the first track can be computed.
    /// Note that the lead-in stored here is the number of samples up to the first index point of the first track, not necessarily to INDEX 01 of the first track;
    /// even the first track may have INDEX 00 data.
    pub leadin_samples: u64,
    /// <1> 1 if the CUESHEET corresponds to a Compact Disc, else 0.
    pub is_cd: bool,
    /// <7+258*8> Reserved. All bits must be set to zero.

    /// <8> The number of tracks.
    /// Must be at least 1 (because of the requisite lead-out track).
    /// For CD-DA, this number must be no more than 100 (99 regular tracks and one lead-out track).
    pub track_number: u8,

    /// One or more tracks.
    /// A CUESHEET block is required to have a lead-out track; it is always the last track in the CUESHEET.
    /// For CD-DA, the lead-out track number must be 170 as specified by the Red Book, otherwise is must be 255.
    pub tracks: Vec<CueSheetTrack>,
}

impl Decode for BlockCueSheet {
    fn from_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let catalog_number = take_string(reader, 128)?;
        let leadin_samples = reader.read_u64::<BigEndian>()?;
        let is_cd = reader.read_u8()? > 0;
        skip(reader, 258)?;
        let track_number = reader.read_u8()?;
        let mut tracks = Vec::with_capacity(track_number as usize);
        for _ in 0..track_number {
            tracks.push(CueSheetTrack::from_reader(reader)?);
        }
        Ok(BlockCueSheet {
            catalog: catalog_number,
            leadin_samples,
            is_cd,
            track_number,
            tracks,
        })
    }
}

#[cfg(feature = "async")]
#[async_trait::async_trait]
impl AsyncDecode for BlockCueSheet {
    async fn from_async_reader<R>(reader: &mut R) -> Result<Self>
    where
        R: AsyncRead + Unpin + Send,
    {
        let catalog_number = take_string_async(reader, 128).await?;
        let leadin_samples = reader.read_u64().await?;
        let is_cd = reader.read_u8().await? > 0;
        skip_async(reader, 258).await?;
        let track_number = reader.read_u8().await?;
        let mut tracks = Vec::with_capacity(track_number as usize);
        for _ in 0..track_number {
            tracks.push(CueSheetTrack::from_async_reader(reader).await?);
        }
        Ok(BlockCueSheet {
            catalog: catalog_number,
            leadin_samples,
            is_cd,
            track_number,
            tracks,
        })
    }
}

impl Encode for BlockCueSheet {
    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        let padding = 128 - self.catalog.len();
        writer.write_all(self.catalog.as_bytes())?;
        writer.write_all(&vec![0u8; padding])?;
        writer.write_u64::<BigEndian>(self.leadin_samples)?;
        writer.write_u8(if self.is_cd { 0b10000000 } else { 0 })?;
        writer.write_all(&[0; 258])?;

        writer.write_u8(self.track_number)?;
        for track in self.tracks.iter() {
            track.write_to(writer)?;
        }
        Ok(())
    }
}

impl fmt::Debug for BlockCueSheet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut prefix = "".to_owned();
        if let Some(width) = f.width() {
            prefix = " ".repeat(width);
        }
        writeln!(
            f,
            "{prefix}media catalog number: {}",
            self.catalog,
            prefix = prefix
        )?;
        writeln!(
            f,
            "{prefix}lead-in: {}",
            self.leadin_samples,
            prefix = prefix
        )?;
        writeln!(f, "{prefix}is CD: {}", self.is_cd, prefix = prefix)?;
        writeln!(
            f,
            "{prefix}number of tracks: {}",
            self.track_number,
            prefix = prefix
        )?;
        for (i, t) in self.tracks.iter().enumerate() {
            writeln!(f, "{prefix}{prefix}track[{}]", i, prefix = prefix)?;
            writeln!(
                f,
                "{prefix}{prefix}{prefix}offset: {}",
                t.track_offset,
                prefix = prefix
            )?;
            // TODO: https://github.com/xiph/flac/blob/ce6dd6b5732e319ef60716d9cc9af6a836a4011a/src/metaflac/operations.c#L627-L651
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct CueSheetTrack {
    /// <64> Track offset in samples, relative to the beginning of the FLAC audio stream.
    /// It is the offset to the first index point of the track.
    /// (Note how this differs from CD-DA, where the track's offset in the TOC is that of the track's INDEX 01 even if there is an INDEX 00.)
    /// For CD-DA, the offset must be evenly divisible by 588 samples (588 samples = 44100 samples/sec * 1/75th of a sec).
    pub track_offset: u64,
    /// <8> Track number.
    /// A track number of 0 is not allowed to avoid conflicting with the CD-DA spec, which reserves this for the lead-in.
    /// For CD-DA the number must be 1-99, or 170 for the lead-out; for non-CD-DA, the track number must for 255 for the lead-out.
    /// It is not required but encouraged to start with track 1 and increase sequentially.
    /// Track numbers must be unique within a CUESHEET.
    pub track_number: u8,
    /// <12*8> Track ISRC.
    /// This is a 12-digit alphanumeric code; see here and here.
    /// A value of 12 ASCII NUL characters may be used to denote absence of an ISRC.
    pub isrc: [u8; 12],
    /// <1> The track type: 0 for audio, 1 for non-audio.
    /// This corresponds to the CD-DA Q-channel control bit 3.
    pub is_audio: bool,
    /// <1> The pre-emphasis flag: 0 for no pre-emphasis, 1 for pre-emphasis.
    /// This corresponds to the CD-DA Q-channel control bit 5; see [here](http://www.chipchapin.com/CDMedia/cdda9.php3).
    pub is_pre_emphasis: bool,
    /// <6+13*8> Reserved. All bits must be set to zero.

    /// <8> The number of track index points.
    /// There must be at least one index in every track in a CUESHEET except for the lead-out track, which must have zero.
    /// For CD-DA, this number may be no more than 100.
    pub index_point_number: u8,

    /// For all tracks except the lead-out track, one or more track index points.
    pub track_index: Vec<CueSheetTrackIndex>,
}

impl Decode for CueSheetTrack {
    fn from_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let track_offset = reader.read_u64::<BigEndian>()?;
        let track_number = reader.read_u8()?;
        let mut isrc = [0u8; 12];
        reader.read_exact(&mut isrc)?;

        let b = reader.read_u8()?;
        let is_audio = (b & 0b10000000) > 0;
        let is_pre_emphasis = (b & 0b01000000) > 0;
        skip(reader, 13)?;

        let index_point_number = reader.read_u8()?;
        let mut track_index = Vec::with_capacity(index_point_number as usize);
        for _ in 0..index_point_number {
            track_index.push(CueSheetTrackIndex::from_reader(reader)?);
        }

        Ok(CueSheetTrack {
            track_offset,
            track_number,
            isrc,
            is_audio,
            is_pre_emphasis,
            index_point_number,
            track_index,
        })
    }
}

#[cfg(feature = "async")]
#[async_trait::async_trait]
impl AsyncDecode for CueSheetTrack {
    async fn from_async_reader<R>(reader: &mut R) -> Result<Self>
    where
        R: AsyncRead + Unpin + Send,
    {
        let track_offset = reader.read_u64().await?;
        let track_number = reader.read_u8().await?;
        let mut isrc = [0u8; 12];
        reader.read_exact(&mut isrc).await?;

        let b = reader.read_u8().await?;
        let is_audio = (b & 0b10000000) > 0;
        let is_pre_emphasis = (b & 0b01000000) > 0;
        skip_async(reader, 13).await?;

        let index_point_number = reader.read_u8().await?;
        let mut track_index = Vec::with_capacity(index_point_number as usize);
        for _ in 0..index_point_number {
            track_index.push(CueSheetTrackIndex::from_async_reader(reader).await?);
        }

        Ok(CueSheetTrack {
            track_offset,
            track_number,
            isrc,
            is_audio,
            is_pre_emphasis,
            index_point_number,
            track_index,
        })
    }
}

impl Encode for CueSheetTrack {
    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_u64::<BigEndian>(self.track_offset)?;
        writer.write_u8(self.track_number)?;
        writer.write_all(&self.isrc)?;

        let b = if self.is_audio { 0b10000000 } else { 0 }
            + if self.is_pre_emphasis { 0b01000000 } else { 0 };
        writer.write_u8(b)?;
        writer.write_all(&[0; 13])?;

        writer.write_u8(self.index_point_number)?;
        for index in self.track_index.iter() {
            index.write_to(writer)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct CueSheetTrackIndex {
    /// <64> Offset in samples, relative to the track offset, of the index point.
    /// For CD-DA, the offset must be evenly divisible by 588 samples (588 samples = 44100 samples/sec * 1/75th of a sec).
    /// Note that the offset is from the beginning of the track, not the beginning of the audio data.
    pub sample_offset: u64,
    /// <8> The index point number.
    /// For CD-DA, an index number of 0 corresponds to the track pre-gap.
    /// The first index in a track must have a number of 0 or 1, and subsequently, index numbers must increase by 1.
    /// Index numbers must be unique within a track.
    pub index_point: u8,
    // <3*8> Reserved. All bits must be set to zero.
}

impl Decode for CueSheetTrackIndex {
    fn from_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let sample_offset = reader.read_u64::<BigEndian>()?;
        let index_point = reader.read_u8()?;
        skip(reader, 3)?;
        Ok(CueSheetTrackIndex {
            sample_offset,
            index_point,
        })
    }
}

#[cfg(feature = "async")]
#[async_trait::async_trait]
impl AsyncDecode for CueSheetTrackIndex {
    async fn from_async_reader<R>(reader: &mut R) -> Result<Self>
    where
        R: AsyncRead + Unpin + Send,
    {
        let sample_offset = reader.read_u64().await?;
        let index_point = reader.read_u8().await?;
        skip_async(reader, 3).await?;
        Ok(CueSheetTrackIndex {
            sample_offset,
            index_point,
        })
    }
}

impl Encode for CueSheetTrackIndex {
    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_u64::<BigEndian>(self.sample_offset)?;
        writer.write_u8(self.index_point)?;
        writer.write_all(&[0; 3])?;
        Ok(())
    }
}
