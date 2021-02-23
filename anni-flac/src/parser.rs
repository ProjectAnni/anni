use nom::{IResult, Err, Needed, error};
use nom::number::streaming::{be_u8, be_u16, be_u24, be_u32, be_u64, le_u32};
use nom::lib::std::collections::BTreeMap;
use std::ops::Index;
use crate::stream::Stream;
use crate::blocks::*;

pub type ParseResult<I, O> = Result<O, Err<error::Error<I>>>;

pub fn parse_flac(input: &[u8], parse_frames: Option<bool>) -> ParseResult<&[u8], Stream> {
    if input.len() < 4 {
        return Err(Err::Incomplete(Needed::new(4)));
    }

    let mut header_size = 4;
    let mut metadata_blocks = Vec::new();

    // Magic number
    let (mut remaining, _) = tag!(input, "fLaC")?;
    loop {
        let (_remaining, block) = metadata_block(remaining)?;

        let is_last = block.is_last;
        header_size += (block.length + 4) as usize;
        metadata_blocks.push(block);
        remaining = _remaining;

        if is_last {
            break;
        }
    }

    let frames = match parse_frames {
        Some(parse) => {
            if parse {
                // FIXME: unimplemented
                Frames::Parsed(vec![])
            } else {
                Frames::Unparsed(input[header_size..].to_vec())
            }
        }
        None => Frames::Skip,
    };

    Ok(Stream {
        header_size,
        metadata_blocks,
        frames,
    })
}

#[derive(Debug)]
pub struct MetadataBlock {
    pub is_last: bool,
    pub length: u32,
    pub data: MetadataBlockData,
}

impl MetadataBlock {
    pub fn print(&self, i: usize) {
        let data = &self.data;
        println!("METADATA block #{}", i);
        println!("  type: {} ({})", u8::from(data), data.as_str());
        println!("  is last: {}", &self.is_last);
        println!("  length: {}", &self.length);
        match data {
            MetadataBlockData::StreamInfo(s) => {
                println!("  minimum blocksize: {} samples", s.min_block_size);
                println!("  maximum blocksize: {} samples", s.max_block_size);
                println!("  minimum framesize: {} bytes", s.min_frame_size);
                println!("  maximum framesize: {} bytes", s.max_frame_size);
                println!("  sample_rate: {} Hz", s.sample_rate);
                println!("  channels: {}", s.channels);
                println!("  bits-per-sample: {}", s.bits_per_sample);
                println!("  total samples: {}", s.total_samples);
                println!("  MD5 signature: {}", hex::encode(s.md5_signature));
            }
            MetadataBlockData::Application(s) => {
                println!("  application ID: {:x}", s.application_id);
                println!("  data contents:");
                // TODO: hexdump
                println!("  <TODO>");
            }
            MetadataBlockData::SeekTable(s) => {
                println!("  seek points: {}", s.seek_points.len());
                for (i, p) in s.seek_points.iter().enumerate() {
                    if p.is_placehoder() {
                        println!("    point {}: PLACEHOLDER", i);
                    } else {
                        println!("    point {}: sample_number={}, stream_offset={}, frame_samples={}", i, p.sample_number, p.stream_offset, p.frame_samples);
                    }
                }
            }
            MetadataBlockData::VorbisComment(s) => {
                println!("  vendor string: {}", s.vendor_string);
                println!("  comments: {}", s.len());
                for (i, (key, c)) in s.comments.iter().enumerate() {
                    println!("    comment[{}]: {}={}", i, key, c.value());
                }
            }
            MetadataBlockData::CueSheet(s) => {
                println!("  media catalog number: {}", s.catalog_number);
                println!("  lead-in: {}", s.leadin_samples);
                println!("  is CD: {}", s.is_cd);
                println!("  number of tracks: {}", s.track_number);
                for (i, t) in s.tracks.iter().enumerate() {
                    println!("    track[{}]", i);
                    println!("      offset: {}", t.track_offset);
                    // TODO: https://github.com/xiph/flac/blob/ce6dd6b5732e319ef60716d9cc9af6a836a4011a/src/metaflac/operations.c#L627-L651
                }
            }
            MetadataBlockData::Picture(s) => {
                println!("  type: {} ({})", s.picture_type as u8, s.picture_type.as_str());
                println!("  MIME type: {}", s.mime_type);
                println!("  description: {}", s.description);
                println!("  width: {}", s.width);
                println!("  height: {}", s.height);
                println!("  depth: {}", s.depth);
                println!("  colors: {}{}", s.colors, if s.color_indexed() { "" } else { " (unindexed)" });
                println!("  data length: {}", s.data.len());
                println!("  data:");
                // TODO: hexdump
                println!("  <TODO>");
            }
            _ => {}
        }
    }
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
    StreamInfo(BlockStreamInfo),
    Padding,
    Application(BlockApplication),
    SeekTable(BlockSeekTable),
    VorbisComment(MetadataBlockVorbisComment),
    CueSheet(BlockCueSheet),
    Picture(BlockPicture),
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

impl MetadataBlockData {
    pub fn as_str(&self) -> &'static str {
        match self {
            MetadataBlockData::StreamInfo(_) => "STREAMINFO",
            MetadataBlockData::Padding => "PADDING",
            MetadataBlockData::Application(_) => "APPLICATION",
            MetadataBlockData::SeekTable(_) => "SEEKTABLE",
            MetadataBlockData::VorbisComment(_) => "VORBIS_COMMENT",
            MetadataBlockData::CueSheet(_) => "CUESHEET",
            MetadataBlockData::Picture(_) => "PICTURE",
            _ => "INVALID",
        }
    }
}

pub fn block_data(block_type: u8, size: usize) -> impl Fn(&[u8]) -> IResult<&[u8], MetadataBlockData> {
    move |input: &[u8]| {
        match block_type {
            0 => map!(input, |i| metadata_block_stream_info(i), |v| MetadataBlockData::StreamInfo(v)),
            1 => map!(input, |i| take!(i, size), |_| MetadataBlockData::Padding),
            2 => map!(input, |i| metadata_block_application(i, size), |v| MetadataBlockData::Application(v)),
            3 => map!(input, |i| metadata_block_seektable(i, size), |v| MetadataBlockData::SeekTable(v)),
            4 => map!(input, |i| metadata_block_vorbis_comment(i), |v| MetadataBlockData::VorbisComment(v)),
            // 5 => MetadataBlockData::CueSheet,
            6 => map!(input, |i| metadata_block_picture(i), |v| MetadataBlockData::Picture(v)),
            _ => map!(input, |i| take!(i, size), |v| MetadataBlockData::Invalid((block_type, v.to_vec()))),
        }
    }
}

named!(pub metadata_block_stream_info<BlockStreamInfo>, do_parse!(
    min_block_size: be_u16 >>
    max_block_size: be_u16 >>
    min_frame_size: be_u24 >>
    max_frame_size: be_u24 >>
    sample_region: take!(8) >>
    signature: take!(16) >>
    (BlockStreamInfo {
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

pub fn metadata_block_application(input: &[u8], size: usize) -> IResult<&[u8], BlockApplication> {
    if input.len() < size {
        return Err(Err::Incomplete(Needed::new(size)));
    }

    if size < 4 {
        // Application id
        return Err(Err::Incomplete(Needed::new(4)));
    }

    let (_, application_id) = be_u32(input)?;
    Ok((&input[size..], BlockApplication {
        application_id,
        data: Vec::from(&input[4..size]),
    }))
}

pub fn metadata_block_seektable(input: &[u8], size: usize) -> IResult<&[u8], BlockSeekTable> {
    if input.len() < size {
        return Err(Err::Incomplete(Needed::new(size)));
    }

    let mut result: BlockSeekTable = BlockSeekTable { seek_points: Vec::new() };

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
    // [vendor_length] = read an unsigned integer of 32 bits
    // vendor_length: u32,

    /// [vendor_string] = read a UTF-8 vector as [vendor_length] octets
    pub vendor_string: String,

    // [user_comment_list_length] = read an unsigned integer of 32 bits
    // comment_number: u32,

    /// iterate [user_comment_list_length] times
    pub comments: BTreeMap<String, UserComment>,

    // [framing_bit] = read a single bit as boolean
    // if ( [framing_bit] unset or end of packet ) then ERROR
}

impl MetadataBlockVorbisComment {
    pub fn insert(&mut self, comment: UserComment) {
        self.comments.insert(comment.key(), comment);
    }

    pub fn len(&self) -> usize {
        self.comments.len()
    }
}

impl Index<&str> for MetadataBlockVorbisComment {
    type Output = UserComment;

    fn index(&self, index: &str) -> &Self::Output {
        &self.comments[index]
    }
}

impl ToString for MetadataBlockVorbisComment {
    fn to_string(&self) -> String {
        let mut result = String::new();
        for (key, comment) in self.comments.iter() {
            if !result.is_empty() {
                result += "\n";
            }
            result += &format!("{}={}", key, comment.value());
        }
        result
    }
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
        vendor_string: String::from_utf8(vendor_string.to_vec()).expect("Invalid UTF-8 description."),
        comments: comments,
    })
));

pub fn user_comment(input: &[u8], count: u32) -> IResult<&[u8], BTreeMap<String, UserComment>> {
    let mut result = BTreeMap::new();
    let mut remaining = input;
    let mut offset: usize = 0;
    for _i in 0..count {
        let (_remaining, length) = le_u32(remaining)?;
        let (_remaining, comment) = take!(_remaining, length as usize)?;
        let comment = String::from_utf8(comment.to_vec()).expect("Invalid UTF-8 description.");
        let comment = UserComment::new(comment);
        // NOT override only when key exists AND comment.value is EMPTY.
        if !(result.contains_key(&comment.key()) && comment.value().len() == 0) {
            result.insert(comment.key(), comment);
        }
        offset += (length + 4) as usize;
        remaining = _remaining;
    }

    Ok((&input[offset..], result))
}

named!(pub metadata_block_picture<BlockPicture>, do_parse!(
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
    (BlockPicture {
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
        mime_type: String::from_utf8(mime_type.to_vec()).expect("Invalid UTF-8 description."),
        description: String::from_utf8(description.to_vec()).expect("Invalid UTF-8 description."),
        width: width,
        height: height,
        depth: color_depth,
        colors: number_of_colors,
        data: picture_data.to_vec(),
    })
));

#[derive(Debug)]
pub enum Frames {
    Parsed(Vec<Frame>),
    Unparsed(Vec<u8>),
    Skip,
}

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
    pub sample_rate: SampleRate,
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
pub enum SampleRate {
    Inherit,
    Rate88200,
    Rate176400,
    Rate192000,
    Rate8000,
    Rate16000,
    Rate22050,
    Rate24000,
    Rate32000,
    Rate44100,
    Rate48000,
    Rate96000,
    Custom(u64),
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
    /// <n> Unencoded warm-up samples (n = frame's bits-per-sample * predictor order).
    pub warm_up: Vec<i32>,
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
