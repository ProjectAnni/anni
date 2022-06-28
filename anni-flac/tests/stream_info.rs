use anni_flac::blocks::BlockStreamInfo;

mod common;

#[test]
fn block_stream_info_encode_decode() {
    let block = BlockStreamInfo {
        min_block_size: 4608,
        max_block_size: 4608,
        min_frame_size: 798,
        max_frame_size: 1817,
        sample_rate: 44100,
        channels: 1,
        bits_per_sample: 16,
        total_samples: 44100,
        md5_signature: [
            0xee, 0xc1, 0xef, 0x02, 0x73, 0xe8, 0xc0, 0x26, 0x1e, 0x52, 0x15, 0x9f, 0xc2, 0x13,
            0x67, 0xb0,
        ],
    };
    let info = common::encode_and_decode(&block);
    assert_eq!(format!("{:?}", info), format!("{:?}", block));
}
