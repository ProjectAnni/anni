use anni_flac::decoder::FlacDecoder;
use std::io::Cursor;

#[test]
fn test_decoder_magic_number() {
    FlacDecoder::new(Cursor::new(b"fLaC")).expect("Valid magic number provided but paniced.");
    FlacDecoder::new(Cursor::new(b"fLaD")).err().expect("Invalid magic number provided but no error got.");
}