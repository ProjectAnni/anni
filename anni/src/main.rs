mod flac;

use std::fs::File;
use std::io::Read;
use anni_flac::{parse_flac};

fn main() {
    let mut file = File::open("test.flac").expect("Failed to open file.");
    let mut data = Vec::new();
    file.read_to_end(&mut data).expect("Failed to read file.");
    let (_, stream) = parse_flac(&data).unwrap();
    flac::tags(stream);
}
