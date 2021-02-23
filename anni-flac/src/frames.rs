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
