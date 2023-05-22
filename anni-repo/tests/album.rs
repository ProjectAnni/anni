use anni_repo::prelude::*;
use std::str::FromStr;

fn album_from_str() -> Album {
    Album::from_str(include_str!("fixtures/test-album.toml")).expect("Failed to parse album toml.")
}

#[test]
fn test_serialize_album() {
    let mut album = album_from_str();
    assert_eq!(
        album.format_to_string(),
        include_str!("fixtures/test-album.toml")
    );
}

#[test]
fn test_deserialize_album() {
    let album = album_from_str();
    assert_eq!(
        album.album_id().to_string(),
        "15006392-e2ae-4204-b7db-e59211f3cdcf".to_string()
    );
    assert_eq!(album.full_title(), "夏凪ぎ／宝物になった日【Test】");
    assert_eq!(album.artist(), "やなぎなぎ");
    assert_eq!(album.release_date().to_string(), "2020-12-16");
    assert_eq!(album.track_type().as_ref(), "normal");
    assert_eq!(album.catalog(), "KSLA-0178");

    let tags = album.album_tags();
    assert_eq!(tags[0].name(), "tag1");
    assert_eq!(tags[1].name(), "tag2");

    // TODO: assert for tags
    for disc in album.iter() {
        assert_eq!(disc.catalog(), "KSLA-0178");
        for (i, track) in disc.iter().enumerate() {
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
