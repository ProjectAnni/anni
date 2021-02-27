use std::io::Cursor;
use anni_utils::decode;
use anni_utils::decode::DecodeError;

#[test]
fn take_token() {
    let arr = b"fLaC|2333|114515";
    let mut cursor = Cursor::new(arr);
    assert_eq!(decode::token(&mut cursor, b"fLaC").unwrap(), ());
    assert_eq!(decode::token(&mut cursor, b"|2333|").unwrap(), ());
    assert_eq!(decode::token(&mut cursor, b"114514").map_err(
        |e| match e {
            DecodeError::InvalidTokenError { expected, got } => {
                &expected == b"114514" && &got == b"114515"
            }
            _ => false,
        }), Err(true));
}