use std::io::Read;
use num_traits::FromPrimitive;
use byteorder::{ReadBytesExt, BigEndian};
use crate::utils::{take_string, take};
use crate::prelude::{Decode, Result};

#[derive(Debug)]
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

impl BlockPicture {
    pub fn color_indexed(&self) -> bool {
        self.colors != 0
    }
}

/// The picture type according to the ID3v2 APIC frame:
/// Others are reserved and should not be used. There may only be one each of picture type 1 and 2 in a file.
#[repr(u32)]
#[derive(Copy, Clone, Debug, FromPrimitive)]
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
