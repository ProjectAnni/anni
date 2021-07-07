use anni_flac::blocks::{UserComment, UserCommentExt, BlockSeekTable, SeekPoint, BlockPicture, PictureType};
use anni_flac::{MetadataBlock, MetadataBlockData};
use std::fs::File;
use std::io::Read;

mod common;

#[test]
fn test_save() {
    let mut header = common::parse_1s_audio();
    header.blocks.insert(1, MetadataBlock::new(MetadataBlockData::SeekTable(BlockSeekTable {
        seek_points: vec![
            SeekPoint {
                sample_number: 0,
                stream_offset: 0,
                frame_samples: 4608,
            }]
    })));

    // Write new metadata
    let comments = header.comments_mut().unwrap();
    comments.vendor_string = "Lavf58.45.100".to_string();
    comments.clear();
    comments.push(UserComment::title("TRACK ONE"));
    comments.push(UserComment::album("TestAlbum"));
    comments.push(UserComment::artist("TestArtist"));
    comments.push(UserComment::date("2021-01-24"));
    comments.push(UserComment::track_number(1));
    comments.push(UserComment::track_total(1));
    comments.push(UserComment::disc_number(1));
    comments.push(UserComment::disc_total(1));

    let mut cover_buf = Vec::new();
    File::open("../assets/1s-cover.png").unwrap().read_to_end(&mut cover_buf).unwrap();
    header.blocks.push(MetadataBlock::new(MetadataBlockData::Picture(BlockPicture {
        picture_type: PictureType::CoverFront,
        mime_type: "image/png".to_string(),
        description: "".to_string(),
        width: 640,
        height: 480,
        depth: 24,
        colors: 0,
        data: cover_buf,
    })));

    let file = tempfile::NamedTempFile::new().unwrap().into_temp_path();
    header.save(Some(file)).unwrap();
    //TODO: assert(file == 1s-full)
}