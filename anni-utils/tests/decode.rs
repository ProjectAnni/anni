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

#[test]
fn u32_le() -> Result<(), decode::DecodeError> {
    let arr = vec![1, 2, 3, 4, 5, 6, 7, 8];
    let mut cursor = Cursor::new(arr);
    assert_eq!(decode::u32_le(&mut cursor)?, 0x04030201);
    assert_eq!(decode::u32_le(&mut cursor)?, 0x08070605);
    Ok(())
}

#[test]
fn u32_be() -> Result<(), decode::DecodeError> {
    let arr = vec![1, 2, 3, 4, 5, 6, 7, 8];
    let mut cursor = Cursor::new(arr);
    assert_eq!(decode::u32_be(&mut cursor)?, 0x01020304);
    assert_eq!(decode::u32_be(&mut cursor)?, 0x05060708);
    Ok(())
}
