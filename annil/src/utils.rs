use axum::response::{IntoResponse, IntoResponseParts};

pub(crate) enum Either<L, R> {
    Left(L),
    Right(R),
}

impl<L, R> IntoResponseParts for Either<L, R>
where
    L: IntoResponseParts,
    R: IntoResponseParts,
{
    type Error = Either<L::Error, R::Error>;

    fn into_response_parts(
        self,
        res: axum::response::ResponseParts,
    ) -> Result<axum::response::ResponseParts, Self::Error> {
        match self {
            Either::Left(l) => l.into_response_parts(res).map_err(|e| Either::Left(e)),
            Either::Right(r) => r.into_response_parts(res).map_err(|e| Either::Right(e)),
        }
    }
}

impl<L, R> IntoResponse for Either<L, R>
where
    L: IntoResponse,
    R: IntoResponse,
{
    fn into_response(self) -> axum::response::Response {
        match self {
            Either::Left(l) => l.into_response(),
            Either::Right(r) => r.into_response(),
        }
    }
}

/// Calculate output size of opus file
pub fn opus_file_size(milliseconds: u64, bit_rate: u16, frame_size: u8) -> u64 {
    const OGG_PREFIX_PAGES_SIZE: u64 = 0x2f + 0x31a;
    const FIXED_OGG_PAGE_HEADER_SIZE: u64 = 26 + 1;
    const MAX_DELAY: u64 = 1000;

    // 110ms, frame_size = 20, produces 6 packets
    // 120ms, frame_size = 20, produces 7 packets
    let total_opus_packets = (milliseconds / frame_size as u64) + 1;
    let total_ogg_pages = total_opus_packets.div_ceil(MAX_DELAY / frame_size as u64);

    let opus_packet_size = bit_rate as u64 * frame_size as u64 / 8;
    let opus_packages_per_ogg_page = opus_packet_size.div_ceil(0xff);

    OGG_PREFIX_PAGES_SIZE
        + total_ogg_pages * FIXED_OGG_PAGE_HEADER_SIZE
        + opus_packages_per_ogg_page * total_opus_packets
        + total_opus_packets * opus_packet_size
}

#[cfg(test)]
mod tests {
    use crate::utils::opus_file_size;

    #[test]
    fn test_sparkle_opus_size() {
        // data generated by transcoding [220617][MVC-0064] Animelo Summer Live 2022 -Sparkle- テーマソング
        assert_eq!(opus_file_size(248745, 64, 60), 2006233);
        assert_eq!(opus_file_size(248745, 128, 60), 4004605);
        assert_eq!(opus_file_size(248745, 192, 60), 6002977);
        assert_eq!(opus_file_size(248745, 256, 60), 8001349);
    }

    #[test]
    fn test_sakuranotoki_opus_size() {
        assert_eq!(opus_file_size(80361372 * 1000 / 44100, 64, 20), 14719255);
    }

    #[test]
    fn test_silence_opus_size() {
        // New logical stream (#1, serial: 5219954f): type opus
        // Encoded with libopus 1.4, libopusenc 0.2.1
        // User comments section follows...
        // 	ENCODER=opusenc from opus-tools 0.2
        // 	ENCODER_OPTIONS=--bitrate 64 --hard-cbr --music --comp 0 --discard-comments --discard-pictures
        // Opus stream 1:
        // 	Pre-skip: 312
        // 	Playback gain: 0 dB
        // 	Channels: 2
        // 	Original sample rate: 48000 Hz
        // 	Packet duration:   20.0ms (max),   20.0ms (avg),   20.0ms (min)
        // 	Page duration:   1000.0ms (max),  610.0ms (avg),  220.0ms (min)
        // 	Total data length: 10716 bytes (overhead: 8.92%)
        // 	Playback length: 0m:01.201s
        // 	Average bitrate: 71.38 kbit/s, w/o overhead: 65.01 kbit/s (hard-CBR)
        // Logical stream 1 ended
        assert_eq!(opus_file_size(1200, 64, 20), 10716);
        assert_eq!(opus_file_size(1201, 64, 20), 10716);

        // New logical stream (#1, serial: 1d1a460c): type opus
        // Encoded with libopus 1.4, libopusenc 0.2.1
        // User comments section follows...
        // 	ENCODER=opusenc from opus-tools 0.2
        // 	ENCODER_OPTIONS=--bitrate 192 --hard-cbr --music --framesize 20 --comp 0 --discard-comments --discard-pictures
        // Opus stream 1:
        // 	Pre-skip: 312
        // 	Playback gain: 0 dB
        // 	Channels: 2
        // 	Original sample rate: 44100 Hz
        // 	Packet duration:   20.0ms (max),   20.0ms (avg),   20.0ms (min)
        // 	Page duration:   1000.0ms (max),  996.5ms (avg),  100.0ms (min)
        // 	Total data length: 6107409 bytes (overhead: 0.54%)
        // 	Playback length: 4m:13.079s
        // 	Average bitrate: 193.1 kbit/s, w/o overhead: 192 kbit/s (hard-CBR)
        // Logical stream 1 ended
        assert_eq!(opus_file_size(253080, 192, 20), 6107409);

        assert_eq!(opus_file_size(1100, 64, 20), 9911);
    }

    #[test]
    fn test_large_packet_size() {
        // For large packets, it should not always use 60ms for the last packet
        // If the last packet actually needs 0~19ms, then use a 20ms packet
        // If the last packet actually needs 20~39ms, then use a 40ms packet
        // If the last packet actually needs 40~59ms, then use a 60ms packet
        // FIXME: replace assert_ne below with assert_eq when the bug is fixed
        assert_eq!(opus_file_size(1200, 64, 60), 10696);
        assert_eq!(opus_file_size(1201, 64, 60), 10696);
        assert_eq!(opus_file_size(1220, 64, 60), 10857); // 1.24
        assert_eq!(opus_file_size(1221, 64, 60), 10857);
        assert_eq!(opus_file_size(1233, 64, 60), 10857);
        assert_eq!(opus_file_size(1234, 64, 60), 11017); // 1.26
        assert_eq!(opus_file_size(1235, 64, 60), 11017);
        assert_eq!(opus_file_size(1240, 64, 60), 11017);
        assert_eq!(opus_file_size(1251, 64, 60), 11017);
        assert_eq!(opus_file_size(1253, 64, 60), 11017);
        assert_eq!(opus_file_size(1254, 64, 60), 11178); // 1.28
        assert_eq!(opus_file_size(1259, 64, 60), 11178);
    }
}
