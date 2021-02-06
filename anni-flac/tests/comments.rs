use anni_flac::{UserComment, parse_flac};
use std::fs::File;
use std::io::Read;

mod common;

#[test]
fn user_comment_lowercase() {
    let c = UserComment::new("a=b".to_string());
    assert_eq!(c.key(), "A");
    assert_eq!(c.key_raw(), "a");
    assert_eq!(c.value(), "b");
    assert_eq!(c.is_key_uppercase(), false);
}

#[test]
fn user_comment_uppercase_key() {
    let c = UserComment::new("A=b".to_string());
    assert_eq!(c.key(), "A");
    assert_eq!(c.key_raw(), "A");
    assert_eq!(c.value(), "b");
    assert_eq!(c.is_key_uppercase(), true);
}

#[test]
fn user_comment_no_equal() {
    let c = UserComment::new("A_WITHOUT_EQUAL".to_string());
    assert_eq!(c.key(), "A_WITHOUT_EQUAL");
    assert_eq!(c.key_raw(), "A_WITHOUT_EQUAL");
    assert_eq!(c.value(), "");
    assert_eq!(c.is_key_uppercase(), true);
}

#[test]
fn user_comment_no_value() {
    let c = UserComment::new("A_WITHOUT_VaLuE=".to_string());
    assert_eq!(c.key(), "A_WITHOUT_VALUE");
    assert_eq!(c.key_raw(), "A_WITHOUT_VaLuE");
    assert_eq!(c.value(), "");
    assert_eq!(c.is_key_uppercase(), false);
}

#[test]
fn test_comments() {
    let stream = common::parse_test_audio();
    stream.comments().expect("Failed to extract comments.");
}