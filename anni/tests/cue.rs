use std::io::Read;

mod common;

#[test]
fn test_cue() {
    // TODO
    let cmd = common::run(&["--help"]).output().unwrap();

    assert_eq!("", String::from_utf8(cmd.out).unwrap());
}