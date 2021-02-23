use anni_flac::{MetadataBlock, MetadataBlockData};
use std::io::Cursor;
use anni_flac::prelude::Decode;

#[test]
fn metadata_block_application() {
    let block = vec![2, 0, 0, 5, 0, 0x99, 0x99, 0xff, 0xfe];
    let mut reader = Cursor::new(block);
    let block = MetadataBlock::from_reader(&mut reader).unwrap();
    assert_eq!(reader.position(), 9);
    assert_eq!(block.is_last, false);
    assert_eq!(block.length, 5);

    assert!(match block.data {
        MetadataBlockData::Application(a) => {
            a.application_id == 0x009999ff && a.data.len() == 1 && a.data[0] == 0xfe
        }
        _ => false,
    });
}
