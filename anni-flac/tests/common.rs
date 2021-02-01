use std::fs::File;
use std::io::Read;
use anni_flac::{parse_flac, MetadataBlockData, PictureType};

/// Make sure test file exists.
///
/// Audio file generated using:
/// ```bash
/// ffmpeg -f lavfi -i "sine=frequency=1000:duration=1" test.flac
/// echo 'TITLE=TRACK ONE
/// ALBUM=TestAlbum
/// ARTIST=TestArtist
/// DATE=2021-01-24
/// TRACKNUMBER=1
/// TRACKTOTAL=1
/// DISCNUMBER=1
/// DISCTOTAL=1' | metaflac --remove-all-tags --import-tags-from=- ./test.flac
/// metaflac --add-seekpoint=1s test.flac
/// ffmpeg -f lavfi -i color=white:640x480:d=3,format=rgb24 -frames:v 1 test.png
/// metaflac --import-picture-from=test.png test.flac
/// ```
#[test]
fn test_audio_file() {
    let exist = std::path::Path::new("tests/test.flac").exists();
    assert!(exist);
}

#[test]
fn test_cover_file() {
    let exist = std::path::Path::new("tests/test.png").exists();
    assert!(exist);
}

#[test]
fn test_audio_tags() {
    let mut file = File::open("../assets/test.flac").expect("Failed to open test flac file.");
    let mut data = Vec::new();
    file.read_to_end(&mut data).expect("Failed to read test flac file.");
    let stream = parse_flac(&data, None).unwrap();
    for (i, block) in stream.metadata_blocks.iter().enumerate() {
        match &block.data {
            MetadataBlockData::StreamInfo(info) => {
                assert_eq!(i, 0);
                assert_eq!(block.is_last, false);
                assert_eq!(block.length, 34);
                assert_eq!(info.min_block_size, 4608);
                assert_eq!(info.max_block_size, 4608);
                assert_eq!(info.min_frame_size, 798);
                assert_eq!(info.max_frame_size, 1317);
                assert_eq!(info.sample_rate, 44100);
                assert_eq!(info.channels, 1);
                assert_eq!(info.bits_per_sample, 16);
                assert_eq!(info.total_samples, 44100);
                assert_eq!(info.md5_signature, [0xee, 0xc1, 0xef, 0x02, 0x73, 0xe8, 0xc0, 0x26, 0x1e, 0x52, 0x15, 0x9f, 0xc2, 0x13, 0x67, 0xb0]);
            }
            MetadataBlockData::SeekTable(table) => {
                assert_eq!(i, 1);
                assert_eq!(block.is_last, false);
                assert_eq!(block.length, 18);
                assert_eq!(table.seek_points.len(), 1);
                assert_eq!(table.seek_points[0].sample_number, 0);
                assert_eq!(table.seek_points[0].stream_offset, 0);
                assert_eq!(table.seek_points[0].frame_samples, 4608);
            }
            MetadataBlockData::VorbisComment(comment) => {
                assert_eq!(i, 2);
                assert_eq!(block.is_last, false);
                assert_eq!(block.length, 163);
                assert_eq!(comment.vendor_string, "Lavf58.45.100");
                assert_eq!(comment.len(), 8);
                assert_eq!(comment["TITLE"].value(), "TRACK ONE");
                assert_eq!(comment["ALBUM"].value(), "TestAlbum");
                assert_eq!(comment["DATE"].value(), "2021-01-24");
                assert_eq!(comment["TRACKNUMBER"].value(), "1");
                assert_eq!(comment["TRACKTOTAL"].value(), "1");
                assert_eq!(comment["DISCNUMBER"].value(), "1");
                assert_eq!(comment["DISCTOTAL"].value(), "1");
            }
            MetadataBlockData::Picture(picture) => {
                assert_eq!(i, 3);
                assert_eq!(block.is_last, false);
                assert_eq!(block.length, 2006);
                assert_eq!(match picture.picture_type {
                    PictureType::CoverFront => true,
                    _ => false,
                }, true);
                assert_eq!(picture.mime_type, "image/png");
                assert_eq!(picture.description, "");
                assert_eq!(picture.width, 640);
                assert_eq!(picture.height, 480);
                assert_eq!(picture.depth, 24);
                assert_eq!(picture.colors, 0);
                assert_eq!(picture.color_indexed(), false);
                assert_eq!(picture.data_length, 1965);

                let mut file = File::open("../assets/test.png").expect("Failed to open cover file.");
                let mut data = Vec::new();
                file.read_to_end(&mut data).expect("Failed to read test cover.");
                assert_eq!(&data, &picture.data);
            }
            MetadataBlockData::Padding => {
                assert_eq!(i, 4);
                assert_eq!(block.is_last, true);
                assert_eq!(block.length, 6043);
            }
            _ => panic!("Invalid block.")
        }
    }
}