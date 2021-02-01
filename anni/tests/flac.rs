use std::io::Read;

mod common;

const TEST_TAGS: &str = r#"ALBUM=TestAlbum
ARTIST=TestArtist
DATE=2021-01-24
DISCNUMBER=1
DISCTOTAL=1
TITLE=TRACK ONE
TRACKNUMBER=1
TRACKTOTAL=1"#;

#[test]
fn flac_export_default() {
    let cmd = common::run(&["flac", "-e", "../assets/test.flac"]).output().unwrap();
    assert_eq!(String::from_utf8(cmd.stdout).expect("Invalid UTF-8 output."), TEST_TAGS);
}

#[test]
fn flac_export_tags() {
    let cmd = common::run(&["flac", "-et=tag", "../assets/test.flac"]).output().unwrap();
    assert_eq!(String::from_utf8(cmd.stdout).expect("Invalid UTF-8 output."), TEST_TAGS);

    let cmd = common::run(&["flac", "-et=comment", "../assets/test.flac"]).output().unwrap();
    assert_eq!(String::from_utf8(cmd.stdout).expect("Invalid UTF-8 output."), TEST_TAGS);
}