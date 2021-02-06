use anni_flac::{metadata_block, MetadataBlockData};

#[test]
fn metadata_block_application() {
    let (_remaining, block) = metadata_block(&[2, 0, 0, 5, 0, 0x99, 0x99, 0xff, 255]).unwrap();
    assert_eq!(_remaining.len(), 0);
    assert_eq!(block.is_last, false);
    assert_eq!(block.length, 5);
    assert_eq!(match block.data {
        MetadataBlockData::Application(data) => {
            data.application_id == 0x009999ff && data.data.len() == 1 && data.data[0] == 255
        }
        _ => false
    }, true);
}
