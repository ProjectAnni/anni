use anni_repo::prelude::*;
use std::str::FromStr;

fn album_from_str() -> Album {
    Album::from_str(include_str!("test-album.toml")).expect("Failed to parse album toml.")
}

#[test]
fn deserialize_album_toml() {
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

#[test]
fn format_one_disc() {
    let mut album = Album::from_str(
        r#"
[album]
album_id = "15006392-e2ae-4204-b7db-e59211f3cdcf"
title = "Title"
artist = "Artist"
date = 2999-12-31
type = "normal"
catalog = "TEST-0001"
tags = ["tag1", "tag2"]

[[discs]]
catalog = "TEST-0001"

[[discs.tracks]]
title = "Track 1"
type = "normal"
artist = "Artist"

[[discs.tracks]]
title = "Track 2"
type = "normal"
artist = "Artist"
"#,
    )
    .expect("Failed to parse album toml.");
    album.format();

    // expect track type to be formatted
    assert_eq!(album.album_type, TrackType::Normal);
    assert_eq!(album.iter().all(|d| d.raw().disc_type == None), true);
    assert_eq!(
        album
            .iter()
            .all(|d| d.iter().all(|t| t.raw().track_type == None)),
        true
    );

    // expect artist to be formatted
    assert_eq!(album.artist, "Artist");
    assert_eq!(album.iter().all(|d| d.raw().artist == None), true);
    assert_eq!(
        album
            .iter()
            .all(|d| d.iter().all(|t| t.raw().artist == None)),
        true
    );
}

#[test]
fn format_multiple_discs() {
    let mut album = Album::from_str(
        r#"
[album]
album_id = "15006392-e2ae-4204-b7db-e59211f3cdcf"
title = "Title"
artist = "Artist"
date = 2999-12-31
type = "normal"
catalog = "TEST-0001~2"
tags = ["tag1", "tag2"]

[[discs]]
catalog = "TEST-0001"

[[discs.tracks]]
title = "Track 1"
type = "normal"
artist = "Artist1"

[[discs.tracks]]
title = "Track 2"
type = "absolute"
artist = "Artist1"

[[discs]]
catalog = "TEST-0002"

[[discs.tracks]]
title = "Track 1"
type = "instrumental"
artist = "Artist1"

[[discs.tracks]]
title = "Track 2"
type = "instrumental"
artist = "Artist2"
"#,
    )
    .expect("Failed to parse album toml.");
    album.format();

    // types
    assert_eq!(album.album_type, TrackType::Normal);
    let disc_types = album
        .iter()
        .map(|d| d.raw().disc_type.clone())
        .collect::<Vec<_>>();
    assert_eq!(disc_types, vec![None, Some(TrackType::Instrumental)]);
    let track_types = album
        .iter()
        .flat_map(|d| {
            d.iter()
                .map(|t| t.raw().track_type.clone())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        track_types,
        vec![None, Some(TrackType::Absolute), None, None]
    );

    // artists
    assert_eq!(album.artist, "Artist");
    let disc_artists = album
        .iter()
        .map(|d| d.raw().artist.clone())
        .collect::<Vec<_>>();
    assert_eq!(disc_artists, vec![Some("Artist1".to_string()), None]);
    let track_artists = album
        .iter()
        .flat_map(|d| d.iter().map(|t| t.raw().artist.clone()).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    assert_eq!(
        track_artists,
        vec![
            None,
            None,
            Some("Artist1".to_string()),
            Some("Artist2".to_string())
        ]
    );
}

#[test]
fn format_overwrite_artist_if_unknown() {
    let mut album = Album::from_str(
        r#"
[album]
album_id = "15006392-e2ae-4204-b7db-e59211f3cdcf"
title = "Title"
artist = "[Unknown Artist]"
date = 2999-12-31
type = "normal"
catalog = "TEST-0001"
tags = ["tag1", "tag2"]

[[discs]]
catalog = "TEST-0001"

[[discs.tracks]]
title = "Track 1"
type = "normal"
artist = "Artist1"

[[discs.tracks]]
title = "Track 2"
type = "normal"
artist = "Artist1"
"#,
    )
    .expect("Failed to parse album toml.");
    album.format();
    assert_eq!(album.artist, "Artist1");
    let disc_artists = album
        .iter()
        .map(|d| d.raw().artist.clone())
        .collect::<Vec<_>>();
    assert_eq!(disc_artists, vec![None]);
    let track_artists = album
        .iter()
        .flat_map(|d| d.iter().map(|t| t.raw().artist.clone()).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    assert_eq!(track_artists, vec![None, None]);
}

#[test]
fn format_do_not_overwrite_artist_if_not_unknown() {
    let mut album = Album::from_str(
        r#"
[album]
album_id = "15006392-e2ae-4204-b7db-e59211f3cdcf"
title = "Title"
artist = "Artist"
date = 2999-12-31
type = "normal"
catalog = "TEST-0001"
tags = ["tag1", "tag2"]

[[discs]]
catalog = "TEST-0001"

[[discs.tracks]]
title = "Track 1"
type = "normal"
artist = "Artist1"

[[discs.tracks]]
title = "Track 2"
type = "normal"
artist = "Artist1"
"#,
    )
    .expect("Failed to parse album toml.");
    album.format();
    assert_eq!(album.artist, "Artist");
    let disc_artists = album
        .iter()
        .map(|d| d.raw().artist.clone())
        .collect::<Vec<_>>();
    assert_eq!(disc_artists, vec![Some("Artist1".to_string())]);
    let track_artists = album
        .iter()
        .flat_map(|d| d.iter().map(|t| t.raw().artist.clone()).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    assert_eq!(track_artists, vec![None, None]);
}

#[test]
fn serialize_album() {
    let mut album = album_from_str();
    assert_eq!(album.format_to_string(), include_str!("test-album.toml"));
}
