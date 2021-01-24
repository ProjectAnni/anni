use anni_flac::UserComment;

#[test]
fn user_comment_lowercase() {
    let c = UserComment::new("a=b".to_string());
    assert_eq!(c.key(), "A");
    assert_eq!(c.key_raw(), "a");
    assert_eq!(c.value_raw(), "b");
    assert_eq!(c.is_key_uppercase(), false);
}

#[test]
fn user_comment_uppercase_key() {
    let c = UserComment::new("A=b".to_string());
    assert_eq!(c.key(), "A");
    assert_eq!(c.key_raw(), "A");
    assert_eq!(c.value_raw(), "b");
    assert_eq!(c.is_key_uppercase(), true);
}

#[test]
fn user_comment_no_equal() {
    let c = UserComment::new("A_WITHOUT_EQUAL".to_string());
    assert_eq!(c.key(), "A_WITHOUT_EQUAL");
    assert_eq!(c.key_raw(), "A_WITHOUT_EQUAL");
    assert_eq!(c.value_raw(), "");
    assert_eq!(c.is_key_uppercase(), true);
}

#[test]
fn user_comment_no_value() {
    let c = UserComment::new("A_WITHOUT_VaLuE=".to_string());
    assert_eq!(c.key(), "A_WITHOUT_VALUE");
    assert_eq!(c.key_raw(), "A_WITHOUT_VaLuE");
    assert_eq!(c.value_raw(), "");
    assert_eq!(c.is_key_uppercase(), false);
}