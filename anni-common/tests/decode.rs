use std::io::Cursor;

use anni_common::decode;
use anni_common::decode::{DecodeError, raw_to_string};

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

#[test]
fn test_raw_to_string() {
    let input = include_bytes!("GNCA-0337.cue");
    let str = raw_to_string(input);
    assert_eq!(str, r#"TITLE "TVアニメ「ご注文はうさぎですか？」キャラクターソング①"
REM DATE "2014"
FILE "GNCA-0337.flac" WAVE
  TRACK 01 AUDIO
    TITLE "全天候型いらっしゃいませ"
    PERFORMER "ココア（佐倉綾音）、チノ（水瀬いのり）"
    INDEX 01 00:00:00
  TRACK 02 AUDIO
    TITLE "ハミングsoon！"
    PERFORMER "ココア（佐倉綾音）"
    INDEX 00 03:44:10
    INDEX 01 03:45:48
  TRACK 03 AUDIO
    TITLE "a cup of happiness"
    PERFORMER "チノ（水瀬いのり）"
    INDEX 00 08:22:22
    INDEX 01 08:23:23
  TRACK 04 AUDIO
    TITLE "全天候型いらっしゃいませ (Instrumental)"
    PERFORMER "ココア（佐倉綾音）、チノ（水瀬いのり）"
    INDEX 00 12:56:41
    INDEX 01 12:58:46
  TRACK 05 AUDIO
    TITLE "ハミングsoon！(Instrumental)"
    PERFORMER "ココア（佐倉綾音）"
    INDEX 00 16:42:18
    INDEX 01 16:43:56
  TRACK 06 AUDIO
    TITLE "a cup of happiness (Instrumental)"
    PERFORMER "チノ（水瀬いのり）"
    INDEX 00 21:20:30
    INDEX 01 21:21:31
"#)
}