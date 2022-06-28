use anni_flac::blocks::{BlockVorbisComment, UserComment};

mod common;

#[test]
fn user_comment_lowercase() {
    let c = UserComment::new("a=b".to_string());
    assert_eq!(c.key(), "A");
    assert_eq!(c.key_raw(), "a");
    assert_eq!(c.value(), "b");
    assert!(!c.is_key_uppercase());
}

#[test]
fn user_comment_uppercase_key() {
    let c = UserComment::new("A=b".to_string());
    assert_eq!(c.key(), "A");
    assert_eq!(c.key_raw(), "A");
    assert_eq!(c.value(), "b");
    assert!(c.is_key_uppercase());
}

#[test]
fn user_comment_no_equal() {
    let c = UserComment::new("A_WITHOUT_EQUAL".to_string());
    assert_eq!(c.key(), "A_WITHOUT_EQUAL");
    assert_eq!(c.key_raw(), "A_WITHOUT_EQUAL");
    assert_eq!(c.value(), "");
    assert!(c.is_key_uppercase());
}

#[test]
fn user_comment_no_value() {
    let c = UserComment::new("A_WITHOUT_VaLuE=".to_string());
    assert_eq!(c.key(), "A_WITHOUT_VALUE");
    assert_eq!(c.key_raw(), "A_WITHOUT_VaLuE");
    assert_eq!(c.value(), "");
    assert!(!c.is_key_uppercase());
}

#[test]
fn user_comment_encode_decode() {
    let comment = BlockVorbisComment {
        vendor_string: "Project Anni".to_string(),
        comments: vec![
            UserComment::new("KEY1=value1".to_string()),
            UserComment::new("KEY2=value2".to_string()),
            UserComment::new("KEY3=".to_string()),
        ],
    };
    let parsed = common::encode_and_decode(&comment);
    assert_eq!(format!("{:?}", parsed), format!("{:?}", comment));
}
