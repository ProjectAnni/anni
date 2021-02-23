use crate::parser::{Frames, MetadataBlock, MetadataBlockVorbisComment, MetadataBlockData};

/// https://xiph.org/flac/format.html
#[derive(Debug)]
pub struct Stream {
    pub(crate) header_size: usize,
    pub metadata_blocks: Vec<MetadataBlock>,
    pub frames: Frames,
}

impl Stream {
    fn block_of(&self, id: u8) -> Option<&MetadataBlock> {
        for block in self.metadata_blocks.iter() {
            if u8::from(&block.data) == id {
                return Some(block);
            }
        }
        None
    }

    pub fn comments(&self) -> Option<&MetadataBlockVorbisComment> {
        self.block_of(4).map(|b| match &b.data {
            MetadataBlockData::VorbisComment(c) => c,
            _ => unreachable!(),
        })
    }
}