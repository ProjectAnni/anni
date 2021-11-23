use std::io::{Read, Write};
use num_traits::FromPrimitive;
use byteorder::{ReadBytesExt, BigEndian, WriteBytesExt};
use crate::utils::*;
use crate::prelude::*;
use std::fmt;
use std::path::Path;
use image::GenericImageView;
use std::borrow::Cow;
use std::str::FromStr;
use crate::error::FlacError;

pub struct BlockPicture {
    /// <32> The picture type according to the ID3v2 APIC frame
    /// Others are reserved and should not be used.
    /// There may only be one each of picture type 1 and 2 in a file.
    pub picture_type: PictureType,
    // <32> The length of the MIME type string in bytes.
    /// <n*8> The MIME type string, in printable ASCII characters 0x20-0x7e.
    /// The MIME type may also be --> to signify that the data part is a URL of the picture instead of the picture data itself.
    pub mime_type: String,
    // <32> The length of the description string in bytes.
    /// <n*8> The description of the picture, in UTF-8.
    pub description: String,
    /// <32> The width of the picture in pixels.
    pub width: u32,
    /// <32> The height of the picture in pixels.
    pub height: u32,
    /// <32> The color depth of the picture in bits-per-pixel.
    pub depth: u32,
    /// <32> For indexed-color pictures (e.g. GIF), the number of colors used, or 0 for non-indexed pictures.
    pub colors: u32,
    // <32> The length of the picture data in bytes.
    /// <n*8> The binary picture data.
    pub data: Vec<u8>,
}

impl Decode for BlockPicture {
    fn from_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let picture_type: PictureType = FromPrimitive::from_u32(reader.read_u32::<BigEndian>()?).unwrap_or(PictureType::Unknown);
        let mime_type_length = reader.read_u32::<BigEndian>()?;
        let mime_type = take_string(reader, mime_type_length as usize)?;
        let description_length = reader.read_u32::<BigEndian>()?;
        let description = take_string(reader, description_length as usize)?;

        let width = reader.read_u32::<BigEndian>()?;
        let height = reader.read_u32::<BigEndian>()?;

        let depth = reader.read_u32::<BigEndian>()?;
        let colors = reader.read_u32::<BigEndian>()?;

        let picture_length = reader.read_u32::<BigEndian>()?;
        let data = take(reader, picture_length as usize)?;
        Ok(BlockPicture {
            picture_type,
            mime_type,
            description,
            width,
            height,
            depth,
            colors,
            data,
        })
    }
}

#[cfg(feature = "async")]
#[async_trait::async_trait]
impl AsyncDecode for BlockPicture {
    async fn from_async_reader<R>(reader: &mut R) -> Result<Self>
        where R: AsyncRead + Unpin + Send
    {
        let picture_type: PictureType = FromPrimitive::from_u32(reader.read_u32().await?).unwrap_or(PictureType::Unknown);
        let mime_type_length = reader.read_u32().await?;
        let mime_type = take_string_async(reader, mime_type_length as usize).await?;
        let description_length = reader.read_u32().await?;
        let description = take_string_async(reader, description_length as usize).await?;

        let width = reader.read_u32().await?;
        let height = reader.read_u32().await?;

        let depth = reader.read_u32().await?;
        let colors = reader.read_u32().await?;

        let picture_length = reader.read_u32().await?;
        let data = take_async(reader, picture_length as usize).await?;
        Ok(BlockPicture {
            picture_type,
            mime_type,
            description,
            width,
            height,
            depth,
            colors,
            data,
        })
    }
}

impl Encode for BlockPicture {
    fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_u32::<BigEndian>(self.picture_type as u32)?;

        writer.write_u32::<BigEndian>(self.mime_type.len() as u32)?;
        writer.write_all(self.mime_type.as_bytes())?;

        writer.write_u32::<BigEndian>(self.description.len() as u32)?;
        writer.write_all(self.description.as_bytes())?;

        writer.write_u32::<BigEndian>(self.width)?;
        writer.write_u32::<BigEndian>(self.height)?;

        writer.write_u32::<BigEndian>(self.depth)?;
        writer.write_u32::<BigEndian>(self.colors)?;

        writer.write_u32::<BigEndian>(self.data.len() as u32)?;
        writer.write_all(&self.data)?;
        Ok(())
    }
}

impl fmt::Debug for BlockPicture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut prefix = "".to_owned();
        if let Some(width) = f.width() {
            prefix = " ".repeat(width);
        }
        writeln!(f, "{prefix}type: {} ({})", self.picture_type as u8, self.picture_type.as_str(), prefix = prefix)?;
        writeln!(f, "{prefix}MIME type: {}", self.mime_type, prefix = prefix)?;
        writeln!(f, "{prefix}description: {}", self.description, prefix = prefix)?;
        writeln!(f, "{prefix}width: {}", self.width, prefix = prefix)?;
        writeln!(f, "{prefix}height: {}", self.height, prefix = prefix)?;
        writeln!(f, "{prefix}depth: {}", self.depth, prefix = prefix)?;
        writeln!(f, "{prefix}colors: {}{}", self.colors, if self.color_indexed() { "" } else { " (unindexed)" }, prefix = prefix)?;
        writeln!(f, "{prefix}data length: {}", self.data.len(), prefix = prefix)?;
        writeln!(f, "{prefix}data:", prefix = prefix)?;
        // TODO: hexdump
        writeln!(f, "{prefix}<TODO>", prefix = prefix)?;
        Ok(())
    }
}

impl BlockPicture {
    pub fn new<P: AsRef<Path>>(file: P, picture_type: PictureType, description: String) -> Result<Self> {
        let img = image::open(file.as_ref())?;
        let mut data = Vec::new();
        std::fs::File::open(file.as_ref())?.read_to_end(&mut data)?;

        let mut ext = file.as_ref().extension().unwrap().to_string_lossy();
        if ext == "jpg" {
            ext = Cow::Borrowed("jpeg");
        }

        Ok(Self {
            picture_type,
            mime_type: format!("image/{}", ext),
            description,
            width: img.width(),
            height: img.height(),
            depth: img.color().bits_per_pixel() as u32,
            colors: 0, // TODO: support format with indexed-color support
            data,
        })
    }

    pub fn color_indexed(&self) -> bool {
        self.colors != 0
    }
}

/// The picture type according to the ID3v2 APIC frame:
/// Others are reserved and should not be used. There may only be one each of picture type 1 and 2 in a file.
#[repr(u32)]
#[derive(Copy, Clone, Debug, FromPrimitive, PartialEq)]
pub enum PictureType {
    /// 0 - Other
    Other,
    /// 1 - 32x32 pixels 'file icon' (PNG only)
    FileIcon,
    /// 2 - Other file icon
    OtherFileIcon,
    /// 3 - Cover (front)
    CoverFront,
    /// 4 - Cover (back)
    CoverBack,
    /// 5 - Leaflet page
    LeafletPage,
    /// 6 - Media (e.g. label side of CD)
    Media,
    /// 7 - Lead artist/lead performer/soloist
    LeadArtist,
    /// 8 - Artist/performer
    Artist,
    /// 9 - Conductor
    Conductor,
    /// 10 - Band/Orchestra
    Band,
    /// 11 - Composer
    Composer,
    /// 12 - Lyricist/text writer
    Lyricist,
    /// 13 - Recording Location
    RecordingLocation,
    /// 14 - During recording
    DuringRecording,
    /// 15 - During performance
    DuringPerformance,
    /// 16 - Movie/video screen capture
    MovieVideoScreenCapture,
    /// 17 - A bright coloured fish
    BrightColoredFish,
    /// 18 - Illustration
    Illustration,
    /// 19 - Band/artist logotype
    BandArtistLogotype,
    /// 20 - Publisher/Studio logotype
    PublisherStudioLogotype,
    /// Unknown Picture Type
    Unknown,
}

impl PictureType {
    pub fn as_str(&self) -> &'static str {
        match self {
            PictureType::Other => "Other",
            PictureType::FileIcon => "32x32 pixels 'file icon' (PNG only)",
            PictureType::OtherFileIcon => "Other file icon",
            PictureType::CoverFront => "Cover (front)",
            PictureType::CoverBack => "Cover (back)",
            PictureType::LeafletPage => "Leaflet page",
            PictureType::Media => "Media (e.g. label side of CD)",
            PictureType::LeadArtist => "Lead artist/lead performer/soloist",
            PictureType::Artist => "Artist/performer",
            PictureType::Conductor => "Conductor",
            PictureType::Band => "Band/Orchestra",
            PictureType::Composer => "Composer",
            PictureType::Lyricist => "Lyricist/text writer",
            PictureType::RecordingLocation => "Recording Location",
            PictureType::DuringRecording => "During recording",
            PictureType::DuringPerformance => "During performance",
            PictureType::MovieVideoScreenCapture => "Movie/video screen capture",
            PictureType::BrightColoredFish => "A bright coloured fish",
            PictureType::Illustration => "Illustration",
            PictureType::BandArtistLogotype => "Band/artist logotype",
            PictureType::PublisherStudioLogotype => "Publisher/Studio logotype",
            PictureType::Unknown => "Unknown",
        }
    }
}

impl FromStr for PictureType {
    type Err = FlacError;

    fn from_str(s: &str) -> core::result::Result<Self, Self::Err> {
        if let Ok(n) = u32::from_str(s) {
            if n <= 20 {
                // n is valid, should not fail
                return Ok(FromPrimitive::from_u32(n).unwrap());
            }
        }

        match s.to_ascii_lowercase().as_str() {
            "other" => Ok(PictureType::Other),
            "file_icon" => Ok(PictureType::FileIcon),
            "other_file_icon" => Ok(PictureType::OtherFileIcon),
            "cover" | "front_cover" => Ok(PictureType::CoverFront),
            "back_cover" => Ok(PictureType::CoverBack),
            "leaflet" => Ok(PictureType::LeafletPage),
            "media" => Ok(PictureType::Media),
            "lead_artist" => Ok(PictureType::LeadArtist),
            "artist" => Ok(PictureType::Artist),
            "conductor" => Ok(PictureType::Conductor),
            "band" => Ok(PictureType::Band),
            "composer" => Ok(PictureType::Composer),
            "lyricist" => Ok(PictureType::Lyricist),
            "recording_location" => Ok(PictureType::RecordingLocation),
            "during_recording" => Ok(PictureType::DuringRecording),
            "during_performance" => Ok(PictureType::DuringPerformance),
            "screen_capture" => Ok(PictureType::MovieVideoScreenCapture),
            "bright_colored_fish" => Ok(PictureType::BrightColoredFish),
            "illustration" => Ok(PictureType::Illustration),
            "band_logo" | "artist_logo" => Ok(PictureType::BandArtistLogotype),
            "publisher_logo" | "studio_logo" => Ok(PictureType::PublisherStudioLogotype),
            &_ => Err(Self::Err::InvalidPictureType),
        }
    }
}
