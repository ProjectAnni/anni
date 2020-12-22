use nom::{IResult, Err, Needed, error};
use nom::number::streaming::{be_u8, be_u16, be_u24, be_u32, be_u64, le_u32};

/// https://xiph.org/flac/format.html
#[derive(Debug)]
pub struct Stream {
    pub metadata_blocks: Vec<MetadataBlock>,
    pub frames: Vec<Frame>,
}

pub type ParseResult<I, O> = Result<O, Err<error::Error<I>>>;

pub fn parse_flac(input: &[u8]) -> ParseResult<&[u8], Stream> {
    if input.len() < 4 {
        return Err(Err::Incomplete(Needed::new(4)));
    }

    // Magic number
    let (mut remaining, _) = tag!(input, "fLaC")?;
    let mut result = Stream {
        metadata_blocks: Vec::new(),
        frames: Vec::new(),
    };
    loop {
        let (_remaining, block) = metadata_block(remaining)?;

        let is_last = block.is_last;
        result.metadata_blocks.push(block);
        remaining = _remaining;

        if is_last {
            break;
        }
    }

    Ok(result)
}

#[derive(Debug)]
pub struct MetadataBlock {
    pub is_last: bool,
    pub length: u32,
    pub data: MetadataBlockData,
}

macro_rules! block_data {
    ($i:expr, $block_type: expr, $size: expr) => ({
        block_data($block_type, $size as usize)($i)
    });
}

named!(pub metadata_block<MetadataBlock>, do_parse!(
    first_byte: be_u8 >>
    length: be_u24 >>
    block: block_data!(first_byte & 0b01111111, length) >>
    (MetadataBlock {
        is_last: first_byte & 0b10000000 > 0,
        length: length,
        data: block,
    })
));

#[derive(Debug)]
pub enum MetadataBlockData {
    StreamInfo(MetadataBlockStreamInfo),
    Padding,
    Application(MetadataBlockApplication),
    SeekTable(MetadataBlockSeekTable),
    VorbisComment(MetadataBlockVorbisComment),
    CueSheet(MetadataBlockCueSheet),
    Picture(MetadataBlockPicture),
    Invalid((u8, Vec<u8>)),
}

impl From<&MetadataBlockData> for u8 {
    fn from(data: &MetadataBlockData) -> Self {
        match data {
            MetadataBlockData::StreamInfo(_) => 0,
            MetadataBlockData::Padding => 1,
            MetadataBlockData::Application(_) => 2,
            MetadataBlockData::SeekTable(_) => 3,
            MetadataBlockData::VorbisComment(_) => 4,
            MetadataBlockData::CueSheet(_) => 5,
            MetadataBlockData::Picture(_) => 6,
            MetadataBlockData::Invalid((t, _)) => *t,
        }
    }
}

impl ToString for MetadataBlockData {
    fn to_string(&self) -> String {
        match self {
            MetadataBlockData::StreamInfo(_) => "STREAMINFO".to_string(),
            MetadataBlockData::Padding => "PADDING".to_string(),
            MetadataBlockData::Application(_) => "APPLICATION".to_string(),
            MetadataBlockData::SeekTable(_) => "SEEKTABLE".to_string(),
            MetadataBlockData::VorbisComment(_) => "VORBIS_COMMENT".to_string(),
            MetadataBlockData::CueSheet(_) => "CUESHEET".to_string(),
            MetadataBlockData::Picture(_) => "PICTURE".to_string(),
            _ => "INVALID".to_string(),
        }
    }
}

pub fn block_data(block_type: u8, size: usize) -> impl Fn(&[u8]) -> IResult<&[u8], MetadataBlockData> {
    move |input: &[u8]| {
        match block_type {
            0 => map!(input, |i| metadata_block_stream_info(i), |v| MetadataBlockData::StreamInfo(v)),
            1 => value!(input, MetadataBlockData::Padding),
            2 => map!(input, |i| metadata_block_application(i, size), |v| MetadataBlockData::Application(v)),
            3 => map!(input, |i| metadata_block_seektable(i, size), |v| MetadataBlockData::SeekTable(v)),
            4 => map!(input, |i| metadata_block_vorbis_comment(i), |v| MetadataBlockData::VorbisComment(v)),
            // 5 => MetadataBlockData::CueSheet,
            6 => map!(input, |i| metadata_block_picture(i), |v| MetadataBlockData::Picture(v)),
            _ => map!(input, |i| take!(i, size), |v| MetadataBlockData::Invalid((block_type, v.to_vec()))),
        }
    }
}

/// Notes:
/// FLAC specifies a minimum block size of 16 and a maximum block size of 65535,
/// meaning the bit patterns corresponding to the numbers 0-15 in the minimum blocksize and maximum blocksize fields are invalid.
#[derive(Debug)]
pub struct MetadataBlockStreamInfo {
    /// <16> The minimum block size (in samples) used in the stream.
    pub min_block_size: u16,
    /// <16> The maximum block size (in samples) used in the stream.
    pub max_block_size: u16,
    /// <24> The minimum frame size (in bytes) used in the stream. May be 0 to imply the value is not known.
    pub min_frame_size: u32,
    /// <24> The maximum frame size (in bytes) used in the stream. May be 0 to imply the value is not known.
    pub max_frame_size: u32,
    /// <20> Sample rate in Hz.
    /// Though 20 bits are available, the maximum sample rate is limited by the structure of frame headers to 655350Hz.
    /// Also, a value of 0 is invalid.
    pub sample_rate: u32,
    /// <3> (number of channels)-1.
    /// FLAC supports from 1 to 8 channels
    pub channels: u8,
    /// <5> (bits per sample)-1.
    /// FLAC supports from 4 to 32 bits per sample.
    /// Currently the reference encoder and decoders only support up to 24 bits per sample.
    pub bits_per_sample: u8,
    /// <36> Total samples in stream.
    /// 'Samples' means inter-channel sample, i.e. one second of 44.1Khz audio will have 44100 samples regardless of the number of channels.
    /// A value of zero here means the number of total samples is unknown.
    pub total_samples: u64,
    /// <128> MD5 signature of the unencoded audio data.
    /// This allows the decoder to determine if an error exists in the audio data even when the error does not result in an invalid bitstream.
    pub md5_signature: [u8; 16],
}

impl MetadataBlockStreamInfo {
    /// (Minimum blocksize == maximum blocksize) implies a fixed-blocksize stream.
    pub fn is_fixed_blocksize_stream(&self) -> bool {
        self.min_block_size == self.max_block_size
    }
}

named!(pub metadata_block_stream_info<MetadataBlockStreamInfo>, do_parse!(
    min_block_size: be_u16 >>
    max_block_size: be_u16 >>
    min_frame_size: be_u24 >>
    max_frame_size: be_u24 >>
    sample_region: take!(8) >>
    signature: take!(16) >>
    (MetadataBlockStreamInfo {
        min_block_size: min_block_size,
        max_block_size: max_block_size,
        min_frame_size: min_frame_size,
        max_frame_size: max_frame_size,
        // 20 bits
        sample_rate: ((sample_region[0] as u32) << 12)
                   + ((sample_region[1] as u32) << 4)
                   + ((sample_region[2] as u32) >> 4),
        // 3 bits
        channels: ((sample_region[2] >> 1) & 0b00000111) + 1,
        // 5 bits
        bits_per_sample: ((sample_region[2] & 0b00000001) << 4) + (sample_region[3] >> 4) + 1,
        // 36 bits
        total_samples: ((sample_region[3] as u64 & 0b00001111) << 32)
                     + ((sample_region[4] as u64) << 24)
                     + ((sample_region[5] as u64) << 16)
                     + ((sample_region[6] as u64) << 8)
                     + (sample_region[7] as u64),
        md5_signature: {
            let mut arr: [u8;16] = Default::default();
            arr.copy_from_slice(signature);
            arr
        },
    })
));

#[derive(Debug)]
pub struct MetadataBlockApplication {
    /// Registered application ID.
    /// (Visit the [registration page](https://xiph.org/flac/id.html) to register an ID with FLAC.)
    pub application_id: u32,
    /// Application data (n must be a multiple of 8)
    pub data: Vec<u8>,
}

pub fn metadata_block_application(input: &[u8], size: usize) -> IResult<&[u8], MetadataBlockApplication> {
    if input.len() < size {
        return Err(Err::Incomplete(Needed::new(size)));
    }

    if size < 4 {
        // Application id
        return Err(Err::Incomplete(Needed::new(4)));
    }

    let (_, application_id) = be_u32(input)?;
    Ok((&input[size..], MetadataBlockApplication {
        application_id,
        data: Vec::from(&input[4..size]),
    }))
}

#[derive(Debug)]
pub struct MetadataBlockSeekTable {
    pub seek_points: Vec<SeekPoint>,
}

/// Notes:
/// - For placeholder points, the second and third field values are undefined.
/// - Seek points within a table must be sorted in ascending order by sample number.
/// - Seek points within a table must be unique by sample number, with the exception of placeholder points.
/// - The previous two notes imply that there may be any number of placeholder points, but they must all occur at the end of the table.
#[derive(Debug)]
pub struct SeekPoint {
    // Sample number of first sample in the target frame, or 0xFFFFFFFFFFFFFFFF for a placeholder point.
    pub sample_number: u64,
    // Offset (in bytes) from the first byte of the first frame header to the first byte of the target frame's header.
    pub stream_offset: u64,
    // Number of samples in the target frame.
    pub frame_samples: u16,
}

impl SeekPoint {
    pub fn is_placehoder(&self) -> bool {
        self.sample_number == 0xFFFFFFFFFFFFFFFF
    }
}

pub fn metadata_block_seektable(input: &[u8], size: usize) -> IResult<&[u8], MetadataBlockSeekTable> {
    if input.len() < size {
        return Err(Err::Incomplete(Needed::new(size)));
    }

    let mut result: MetadataBlockSeekTable = MetadataBlockSeekTable { seek_points: Vec::new() };

    let mut remaining = input;
    // The number of seek points is implied by the metadata header 'length' field, i.e. equal to length / 18.
    let points = size / 18;

    for _ in 0..points {
        let (_remaining, sample_number) = be_u64(remaining)?;
        let (_remaining, stream_offset) = be_u64(_remaining)?;
        let (_remaining, frame_samples) = be_u16(_remaining)?;
        result.seek_points.push(SeekPoint {
            sample_number,
            stream_offset,
            frame_samples,
        });
        remaining = _remaining;
    }

    Ok((&input[size..], result))
}


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
pub struct MetadataBlockVorbisComment {
    /// [vendor_length] = read an unsigned integer of 32 bits
    pub vendor_length: u32,
    /// [vendor_string] = read a UTF-8 vector as [vendor_length] octets
    pub vendor_string: String,
    /// [user_comment_list_length] = read an unsigned integer of 32 bits
    pub comment_number: u32,
    /// iterate [user_comment_list_length] times
    pub comments: Vec<UserComment>,

    // [framing_bit] = read a single bit as boolean
    // if ( [framing_bit] unset or end of packet ) then ERROR
}

#[derive(Debug)]
pub struct UserComment {
    /// [length] = read an unsigned integer of 32 bits
    pub length: u32,
    /// this iteration's user comment = read a UTF-8 vector as [length] octets
    pub comment: String,
}

macro_rules! user_comment {
    ($i:expr, $count: expr) => ({
        user_comment($i, $count)
    });
}

named!(pub metadata_block_vorbis_comment<MetadataBlockVorbisComment>, do_parse!(
    vendor_length: le_u32 >>
    vendor_string: take!(vendor_length) >>
    comment_number: le_u32 >>
    comments: user_comment!(comment_number) >>
    (MetadataBlockVorbisComment {
        vendor_length: vendor_length,
        vendor_string: String::from_utf8(vendor_string.to_vec()).expect("Invalid UTF-8 description."),
        comment_number: comment_number,
        comments: comments,
    })
));

pub fn user_comment(input: &[u8], count: u32) -> IResult<&[u8], Vec<UserComment>> {
    let mut result: Vec<UserComment> = Vec::new();
    let mut remaining = input;
    let mut offset: usize = 0;
    for _i in 0..count {
        let (_remaining, length) = le_u32(remaining)?;
        let (_remaining, comment) = take!(_remaining, length as usize)?;
        result.push(UserComment {
            length,
            comment: String::from_utf8(comment.to_vec()).expect("Invalid UTF-8 description."),
        });
        offset += (length + 4) as usize;
        remaining = _remaining;
    }

    Ok((&input[offset..], result))
}

#[derive(Debug)]
pub struct MetadataBlockCueSheet {
    /// <128*8> Media catalog number, in ASCII printable characters 0x20-0x7e.
    /// In general, the media catalog number may be 0 to 128 bytes long; any unused characters should be right-padded with NUL characters.
    /// For CD-DA, this is a thirteen digit number, followed by 115 NUL bytes.
    pub catalog_number: String,
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
    pub track_isrc: [u8; 12],
    /// <1> The track type: 0 for audio, 1 for non-audio.
    /// This corresponds to the CD-DA Q-channel control bit 3.
    pub is_audio: bool,
    /// <1> The pre-emphasis flag: 0 for no pre-emphasis, 1 for pre-emphasis.
    /// This corresponds to the CD-DA Q-channel control bit 5; see [here](http://www.chipchapin.com/CDMedia/cdda9.php3).
    pub pre_emphasis_flag: bool,
    /// <6+13*8>	Reserved. All bits must be set to zero.

    /// <8> The number of track index points.
    /// There must be at least one index in every track in a CUESHEET except for the lead-out track, which must have zero.
    /// For CD-DA, this number may be no more than 100.
    pub track_index_point_number: u8,

    /// For all tracks except the lead-out track, one or more track index points.
    pub track_index: Vec<CueSheetTrackIndex>,
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

/// The picture type according to the ID3v2 APIC frame:
/// Others are reserved and should not be used. There may only be one each of picture type 1 and 2 in a file.
#[derive(Debug)]
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
}

impl From<&PictureType> for u8 {
    fn from(data: &PictureType) -> Self {
        match data {
            PictureType::Other => 0,
            PictureType::FileIcon => 1,
            PictureType::OtherFileIcon => 2,
            PictureType::CoverFront => 3,
            PictureType::CoverBack => 4,
            PictureType::LeafletPage => 5,
            PictureType::Media => 6,
            PictureType::LeadArtist => 7,
            PictureType::Artist => 8,
            PictureType::Conductor => 9,
            PictureType::Band => 10,
            PictureType::Composer => 11,
            PictureType::Lyricist => 12,
            PictureType::RecordingLocation => 13,
            PictureType::DuringRecording => 14,
            PictureType::DuringPerformance => 15,
            PictureType::MovieVideoScreenCapture => 16,
            PictureType::BrightColoredFish => 17,
            PictureType::Illustration => 18,
            PictureType::BandArtistLogotype => 19,
            PictureType::PublisherStudioLogotype => 20,
        }
    }
}

impl ToString for PictureType {
    fn to_string(&self) -> String {
        match self {
            PictureType::Other => "Other".to_string(),
            PictureType::FileIcon => "32x32 pixels 'file icon' (PNG only)".to_string(),
            PictureType::OtherFileIcon => "Other file icon".to_string(),
            PictureType::CoverFront => "Cover (front)".to_string(),
            PictureType::CoverBack => "Cover (back)".to_string(),
            PictureType::LeafletPage => "Leaflet page".to_string(),
            PictureType::Media => "Media (e.g. label side of CD)".to_string(),
            PictureType::LeadArtist => "Lead artist/lead performer/soloist".to_string(),
            PictureType::Artist => "Artist/performer".to_string(),
            PictureType::Conductor => "Conductor".to_string(),
            PictureType::Band => "Band/Orchestra".to_string(),
            PictureType::Composer => "Composer".to_string(),
            PictureType::Lyricist => "Lyricist/text writer".to_string(),
            PictureType::RecordingLocation => "Recording Location".to_string(),
            PictureType::DuringRecording => "During recording".to_string(),
            PictureType::DuringPerformance => "During performance".to_string(),
            PictureType::MovieVideoScreenCapture => "Movie/video screen capture".to_string(),
            PictureType::BrightColoredFish => "A bright coloured fish".to_string(),
            PictureType::Illustration => "Illustration".to_string(),
            PictureType::BandArtistLogotype => "Band/artist logotype".to_string(),
            PictureType::PublisherStudioLogotype => "Publisher/Studio logotype".to_string(),
        }
    }
}

#[derive(Debug)]
pub struct MetadataBlockPicture {
    /// <32> The picture type according to the ID3v2 APIC frame
    /// Others are reserved and should not be used.
    /// There may only be one each of picture type 1 and 2 in a file.
    pub picture_type: PictureType,
    /// <32> The length of the MIME type string in bytes.
    pub mime_type_length: u32,
    /// <n*8> The MIME type string, in printable ASCII characters 0x20-0x7e.
    /// The MIME type may also be --> to signify that the data part is a URL of the picture instead of the picture data itself.
    pub mime_type: String,
    /// <32> The length of the description string in bytes.
    pub description_length: u32,
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
    /// <32> The length of the picture data in bytes.
    pub data_length: u32,
    /// <n*8> The binary picture data.
    pub data: Vec<u8>,
}

impl MetadataBlockPicture {
    pub fn color_indexed(&self) -> bool {
        self.colors != 0
    }
}

named!(pub metadata_block_picture<MetadataBlockPicture>, do_parse!(
    picture_type: be_u32 >>
    mime_type_length: be_u32 >>
    mime_type: take!(mime_type_length) >>
    description_length: be_u32 >>
    description: take!(description_length) >>
    width: be_u32 >>
    height: be_u32 >>
    color_depth: be_u32 >>
    number_of_colors: be_u32 >>
    picture_length: be_u32 >>
    picture_data: take!(picture_length) >>
    (MetadataBlockPicture {
        picture_type: match picture_type {
            1 => PictureType::FileIcon,
            2 => PictureType::OtherFileIcon,
            3 => PictureType::CoverFront,
            4 => PictureType::CoverBack,
            5 => PictureType::LeafletPage,
            6 => PictureType::Media,
            7 => PictureType::LeadArtist,
            8 => PictureType::Artist,
            9 => PictureType::Conductor,
            10 => PictureType::Band,
            11 => PictureType::Composer,
            12 => PictureType::Lyricist,
            13 => PictureType::RecordingLocation,
            14 => PictureType::DuringRecording,
            15 => PictureType::DuringPerformance,
            16 => PictureType::MovieVideoScreenCapture,
            17 => PictureType::BrightColoredFish,
            18 => PictureType::Illustration,
            19 => PictureType::BandArtistLogotype,
            20 => PictureType::PublisherStudioLogotype,
            _ => PictureType::Other,
        },
        mime_type_length: mime_type_length,
        mime_type: String::from_utf8(mime_type.to_vec()).expect("Invalid UTF-8 description."),
        description_length: description_length,
        description: String::from_utf8(description.to_vec()).expect("Invalid UTF-8 description."),
        width: width,
        height: height,
        depth: color_depth,
        colors: number_of_colors,
        data_length: picture_length,
        data: picture_data.to_vec(),
    })
));

#[derive(Debug)]
pub struct Frame {
    pub header: FrameHeader,

    /// One SUBFRAME per channel.
    pub subframes: Vec<SubFrame>,

    /// FRAME_FOOTER
    /// <16> CRC-16 (polynomial = x^16 + x^15 + x^2 + x^0, initialized with 0) of everything before the crc, back to and including the frame header sync code
    pub crc: u16,
}

#[derive(Debug)]
pub struct FrameHeader {
    // <14> Sync code '11111111111110'
    /// This bit must remain reserved for 0 in order for a FLAC frame's initial 15 bits to be distinguishable from the start of an MPEG audio frame (see also).
    pub reserved: bool,
    /// <1> Blocking strategy:
    /// - 0 : fixed-blocksize stream; frame header encodes the frame number
    /// - 1 : variable-blocksize stream; frame header encodes the sample number
    ///
    /// The "blocking strategy" bit must be the same throughout the entire stream.
    /// The "blocking strategy" bit determines how to calculate the sample number of the first sample in the frame.
    /// If the bit is 0 (fixed-blocksize), the frame header encodes the frame number as above, and the frame's starting sample number will be the frame number times the blocksize.
    /// If it is 1 (variable-blocksize), the frame header encodes the frame's starting sample number itself.
    /// (In the case of a fixed-blocksize stream, only the last block may be shorter than the stream blocksize;
    ///  its starting sample number will be calculated as the frame number times the previous frame's blocksize, or zero if it is the first frame).
    pub block_strategy: BlockStrategy,
    /// <4> Block size in inter-channel samples:
    /// - `0000` : reserved
    /// - `0001` : 192 samples
    /// - `0010-0101` : 576 * (2^(n-2)) samples, i.e. 576/1152/2304/4608
    /// - `0110` : get 8 bit (blocksize-1) from end of header
    /// - `0111` : get 16 bit (blocksize-1) from end of header
    /// - `1000-1111` : 256 * (2^(n-8)) samples, i.e. 256/512/1024/2048/4096/8192/16384/32768
    pub block_size: u16,
    // FIXME: Use enum here
    /// <4> Sample rate:
    /// - `0000` : get from STREAMINFO metadata block
    /// - `0001` : 88.2kHz
    /// - `0010` : 176.4kHz
    /// - `0011` : 192kHz
    /// - `0100` : 8kHz
    /// - `0101` : 16kHz
    /// - `0110` : 22.05kHz
    /// - `0111` : 24kHz
    /// - `1000` : 32kHz
    /// - `1001` : 44.1kHz
    /// - `1010` : 48kHz
    /// - `1011` : 96kHz
    /// - `1100` : get 8 bit sample rate (in kHz) from end of header
    /// - `1101` : get 16 bit sample rate (in Hz) from end of header
    /// - `1110` : get 16 bit sample rate (in tens of Hz) from end of header
    /// - `1111` : invalid, to prevent sync-fooling string of 1s
    pub sample_rate: Option<u32>,
    /// <4> Channel assignment
    /// - `0000-0111` : (number of independent channels)-1. Where defined, the channel order follows SMPTE/ITU-R recommendations. The assignments are as follows:
    ///   - 1 channel: mono
    ///   - 2 channels: left, right
    ///   - 3 channels: left, right, center
    ///   - 4 channels: front left, front right, back left, back right
    ///   - 5 channels: front left, front right, front center, back/surround left, back/surround right
    ///   - 6 channels: front left, front right, front center, LFE, back/surround left, back/surround right
    ///   - 7 channels: front left, front right, front center, LFE, back center, side left, side right
    ///   - 8 channels: front left, front right, front center, LFE, back left, back right, side left, side right
    /// - `1000` : left/side stereo: channel 0 is the left channel, channel 1 is the side(difference) channel
    /// - `1001` : right/side stereo: channel 0 is the side(difference) channel, channel 1 is the right channel
    /// - `1010` : mid/side stereo: channel 0 is the mid(average) channel, channel 1 is the side(difference) channel
    /// - `1011-1111` : reserved
    pub channel_assignment: ChannelAssignment,
    /// <3> Sample size in bits:
    /// `000` : get from STREAMINFO metadata block
    /// `001` : 8 bits per sample
    /// `010` : 12 bits per sample
    /// `011` : reserved
    /// `100` : 16 bits per sample
    /// `101` : 20 bits per sample
    /// `110` : 24 bits per sample
    /// `111` : reserved
    pub sample_size: Option<u8>,
    // <?> if(blocksize bits == 011x) 8/16 bit (blocksize-1)
    // <?> if(sample rate bits == 11xx) 8/16 bit sample rate
    /// <8> CRC-8 (polynomial = x^8 + x^2 + x^1 + x^0, initialized with 0) of everything before the crc, including the sync code
    pub crc: u8,
}

#[derive(Debug)]
pub enum BlockStrategy {
    /// <8-48>:"UTF-8" coded frame number (decoded number is 31 bits)
    Fixed(u32),
    /// <8-56>:"UTF-8" coded sample number (decoded number is 36 bits)
    Variable(u64),
}

#[derive(Debug)]
pub enum ChannelAssignment {
    Independent(u8),
    LeftSide,
    RightSide,
    MidSide,
    Reserved(u8),
}

#[derive(Debug)]
pub struct SubFrame {
    // <1> Zero bit padding, to prevent sync-fooling string of 1s
    /// <6> Subframe type:
    /// `000000` : SUBFRAME_CONSTANT
    /// `000001` : SUBFRAME_VERBATIM
    /// `00001x` : reserved
    /// `0001xx` : reserved
    /// `001xxx` : if(xxx <= 4) SUBFRAME_FIXED, xxx=order ; else reserved
    /// `01xxxx` : reserved
    /// `1xxxxx` : SUBFRAME_LPC, xxxxx=order-1
    pub content: SubframeType,
    /// <1+k> 'Wasted bits-per-sample' flag:
    /// - `0` : no wasted bits-per-sample in source subblock, k=0
    /// - `1` : k wasted bits-per-sample in source subblock, k-1 follows, unary coded; e.g. k=3 => 001 follows, k=7 => 0000001 follows.
    pub wasted_bits: u32,
}

#[derive(Debug)]
pub enum SubframeType {
    /// <n> Unencoded constant value of the subblock, n = frame's bits-per-sample.
    Constant(i32),
    Fixed(SubframeFixed),
    LPC(SubframeLPC),
    /// <n*i> Unencoded subblock; n = frame's bits-per-sample, i = frame's blocksize.
    Verbatim(Vec<u8>),
}

#[derive(Debug)]
pub struct SubframeFixed {
    // TODO: check type
    pub warm_up: u16,
    /// Encoded residual
    pub residual: Residual,
}

#[derive(Debug)]
pub struct SubframeLPC {
    /// <n> Unencoded warm-up samples (n = frame's bits-per-sample * lpc order).
    pub warm_up: Vec<i32>,
    /// <4> (Quantized linear predictor coefficients' precision in bits)-1 (1111 = invalid).
    pub qlp_coeff_prediction: u8,
    /// <5> Quantized linear predictor coefficient shift needed in bits (NOTE: this number is signed two's-complement).
    pub qlp_shift: u8,
    /// <n> Unencoded predictor coefficients (n = qlp coeff precision * lpc order) (NOTE: the coefficients are signed two's-complement).
    pub qlp_coeff: Vec<u8>,
    /// Encoded residual
    pub residual: Residual,
}

/// <2> Residual coding method:
/// `00` : partitioned Rice coding with 4-bit Rice parameter;
///      RESIDUAL_CODING_METHOD_PARTITIONED_RICE follows
/// `01` : partitioned Rice coding with 5-bit Rice parameter;
///      RESIDUAL_CODING_METHOD_PARTITIONED_RICE2 follows
/// `10-11` : reserved
#[derive(Debug)]
pub enum Residual {
    Rice(ResidualCodingMethodPartitionedRice),
    Rice2(ResidualCodingMethodPartitionedRice),
    /// false: 10
    /// true: 11
    Reserved(bool),
}

#[derive(Debug)]
pub struct ResidualCodingMethodPartitionedRice {
    /// <4> Partition order.
    pub order: u8,
    /// <RICE_PARTITION+> There will be 2^order partitions.
    pub partitons: Vec<RicePartition>,
}

#[derive(Debug)]
pub struct RicePartition {
    /// Encoding parameter:
    pub parameter: RiceParameter,
    /// Encoded residual. The number of samples (n) in the partition is determined as follows:
    /// - if the partition order is zero, n = frame's blocksize - predictor order
    /// - else if this is not the first partition of the subframe, n = (frame's blocksize / (2^partition order))
    /// - else n = (frame's blocksize / (2^partition order)) - predictor order
    pub encoded_residual: Vec<u8>,
}

#[derive(Debug)]
pub enum RiceParameter {
    /// Rice:
    /// <4> `0000-1110` : Rice parameter.
    /// Rice2:
    /// <5> `00000-11110` : Rice parameter.
    Parameter(u8),
    /// Rice:
    /// <4+5> `1111` : Escape code, meaning the partition is in unencoded binary form using n bits per sample; n follows as a 5-bit number.
    /// Rice2:
    /// <5+5> `11111` : Escape code, meaning the partition is in unencoded binary form using n bits per sample; n follows as a 5-bit number.
    /// n is stored
    Escape(u8),
}

#[cfg(test)]
mod tests {
    use crate::parser::{metadata_block, MetadataBlockData, parse_flac};
    use std::io::Read;

    #[test]
    fn metadata_block_application() {
        let (_remaining, block) = metadata_block(&[2, 0, 0, 5, 0, 0x99, 0x99, 0xff, 255]).unwrap();
        assert_eq!(_remaining.len(), 0);
        assert_eq!(block.is_last, false);
        assert_eq!(block.length, 5);
        assert!(match block.data {
            MetadataBlockData::Application(data) => {
                data.application_id == 0x009999ff && data.data.len() == 1 && data.data[0] == 255
            }
            _ => false
        });
    }

    #[test]
    fn parse_file() {
        use std::fs::File;
        let mut file = File::open("test.flac").expect("Failed to open file.");
        let mut data = Vec::new();
        file.read_to_end(&mut data).expect("Failed to read file.");
        let _stream = parse_flac(&data).unwrap();
    }
}
