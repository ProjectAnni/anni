use anni_flac::{MetadataBlock, MetadataBlockData};
use std::io::Cursor;
use anni_flac::prelude::{Decode, Encode};

#[test]
fn block_application_encode_decode() {
    let block = vec![2, 0, 0, 5, 0, 0x99, 0x99, 0xff, 0xfe];
    let mut reader = Cursor::new(block);
    let block = MetadataBlock::from_reader(&mut reader).unwrap();
    assert_eq!(reader.position(), 9);

    let buf = Vec::new();
    let mut buf = Cursor::new(buf);
    block.write_to(&mut buf).expect("Failed to write to buf");
    assert_eq!(reader.into_inner(), buf.into_inner());

    // assert_eq!(block.is_last, false);
    // assert_eq!(block.length, 5);

    assert!(match block.data {
        MetadataBlockData::Application(a) => {
            a.application_id == 0x009999ff && a.data.len() == 1 && a.data[0] == 0xfe
        }
        _ => false,
    });
}
