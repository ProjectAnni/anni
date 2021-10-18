use anni_repo::album::{Album, TrackType};
use std::str::FromStr;

fn album_from_str() -> Album {
    Album::from_str(r#"
[album]
title = "夏凪ぎ/宝物になった日"
edition = "Test"
artist = "やなぎなぎ"
date = 2020-12-16
type = "normal"
catalog = "KSLA-0178"
tags = ["tag1", "tag2:e1", "tag3:special:e2", { "name" = "tag3", "edition" = "e2" }]

[[discs]]
catalog = "KSLA-0178"

[[discs.tracks]]
title = "夏凪ぎ"
artist = "やなぎなぎ"

[[discs.tracks]]
title = "宝物になった日"

[[discs.tracks]]
title = "夏凪ぎ(Episode 9 Ver.)"

[[discs.tracks]]
title = "宝物になった日(Episode 5 Ver.)"

[[discs.tracks]]
title = "夏凪ぎ(Instrumental)"
artist = "麻枝准"
type = "instrumental"

[[discs.tracks]]
title = "宝物になった日(Instrumental)"
artist = "麻枝准"
type = "instrumental"
"#).expect("Failed to parse album toml.")
}

#[test]
fn deserialize_album_toml() {
    let album = album_from_str();
    assert_eq!(album.title(), "夏凪ぎ/宝物になった日【Test】");
    assert_eq!(album.artist(), "やなぎなぎ");
    assert_eq!(album.release_date().to_string(), "2020-12-16");
    assert_eq!(album.track_type().as_ref(), "normal");
    assert_eq!(album.catalog(), "KSLA-0178");

    // TODO: assert for tags
    for disc in album.discs() {
        assert_eq!(disc.catalog(), "KSLA-0178");
        for (i, track) in disc.tracks().iter().enumerate() {
            match i {
                0 => {
                    assert_eq!(track.title(), "夏凪ぎ");
                    assert_eq!(track.artist(), "やなぎなぎ");
                    assert!(matches!(track.track_type(), TrackType::Normal));
                }
                1 => {
                    assert_eq!(track.title(), "宝物になった日");
                    assert_eq!(track.artist(), "やなぎなぎ");
                    assert!(matches!(track.track_type(), TrackType::Normal));
                }
                2 => {
                    assert_eq!(track.title(), "夏凪ぎ(Episode 9 Ver.)");
                    assert_eq!(track.artist(), "やなぎなぎ");
                    assert!(matches!(track.track_type(), TrackType::Normal));
                }
                3 => {
                    assert_eq!(track.title(), "宝物になった日(Episode 5 Ver.)");
                    assert_eq!(track.artist(), "やなぎなぎ");
                    assert!(matches!(track.track_type(), TrackType::Normal));
                }
                4 => {
                    assert_eq!(track.title(), "夏凪ぎ(Instrumental)");
                    assert_eq!(track.artist(), "麻枝准");
                    assert!(matches!(track.track_type(), TrackType::Instrumental));
                }
                5 => {
                    assert_eq!(track.title(), "宝物になった日(Instrumental)");
                    assert_eq!(track.artist(), "麻枝准");
                    assert!(matches!(track.track_type(), TrackType::Instrumental));
                }
                _ => unreachable!(),
            }
        }
    }
}

#[test]
fn serialize_album() {
    let album = album_from_str();
    assert_eq!(album.to_string(), r#"[album]
title = "夏凪ぎ/宝物になった日"
artist = "やなぎなぎ"
date = 2020-12-16
type = "normal"
catalog = "KSLA-0178"

[[discs]]
catalog = "KSLA-0178"

[[discs.tracks]]
title = "夏凪ぎ"
artist = "やなぎなぎ"

[[discs.tracks]]
title = "宝物になった日"

[[discs.tracks]]
title = "夏凪ぎ(Episode 9 Ver.)"

[[discs.tracks]]
title = "宝物になった日(Episode 5 Ver.)"

[[discs.tracks]]
title = "夏凪ぎ(Instrumental)"
artist = "麻枝准"
type = "instrumental"

[[discs.tracks]]
title = "宝物になった日(Instrumental)"
artist = "麻枝准"
type = "instrumental"
"#);
}
