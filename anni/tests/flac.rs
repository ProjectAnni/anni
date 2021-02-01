use std::io::Read;
use std::fs::File;

mod common;

const TEST_TAGS: &str = r#"ALBUM=TestAlbum
ARTIST=TestArtist
DATE=2021-01-24
DISCNUMBER=1
DISCTOTAL=1
TITLE=TRACK ONE
TRACKNUMBER=1
TRACKTOTAL=1
"#;

const FLAC_PATH: &str = "../assets/test.flac";
const COVER_PATH: &str = "../assets/test.png";

#[test]
fn flac_export_default() {
    let cmd = common::run(&["flac", "-e", FLAC_PATH]).output().unwrap();
    assert_eq!(String::from_utf8(cmd.stdout).expect("Invalid UTF-8 output."), TEST_TAGS);
}

#[test]
fn flac_export_tags() {
    let cmd = common::run(&["flac", "-et=tag", FLAC_PATH]).output().unwrap();
    assert_eq!(String::from_utf8(cmd.stdout).expect("Invalid UTF-8 output."), TEST_TAGS);

    let cmd = common::run(&["flac", "-et=comment", FLAC_PATH]).output().unwrap();
    assert_eq!(String::from_utf8(cmd.stdout).expect("Invalid UTF-8 output."), TEST_TAGS);
}

#[test]
fn flac_export_cover() {
    let cmd = common::run(&["flac", "-et=cover", FLAC_PATH]).output().unwrap();

    let mut file = File::open(COVER_PATH).expect("Failed to open cover.");
    let mut data = Vec::new();
    file.read_to_end(&mut data).expect("Failed to read cover.");
    assert_eq!(cmd.stdout, data);
}