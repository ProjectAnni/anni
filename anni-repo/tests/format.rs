use anni_repo::prelude::{Album, TrackType};
use std::str::FromStr;

macro_rules! format_test {
    ($path: expr) => {
        let mut album = Album::from_str(include_str!(concat!(
            "fixtures/format/",
            $path,
            "/unformatted.toml"
        )))
        .unwrap();
        let formatted = album.format_to_string();
        let expected = include_str!(concat!("fixtures/format/", $path, "/formatted.toml"));
        assert_eq!(formatted, expected);
    };
}

#[test]
fn test_format_track_artist_to_disc_artist() {
    format_test!("track-artist-to-disc-artist");
}

#[test]
fn test_format_track_type_to_disc_type() {
    format_test!("track-type-to-disc-type");
}

#[test]
fn test_format_disc_artist_to_album_artist_on_unknown() {
    format_test!("disc-artist-to-album-artist-on-unknown");
}
#[test]
fn test_format_disc_artist_to_album_artist_on_not_unknown() {
    format_test!("disc-artist-to-album-artist-on-not-unknown");
}

#[test]
fn test_format_disc_type_to_album_type() {
    format_test!("disc-type-to-album-type");
}

#[test]
fn test_format_overall() {
    format_test!("overall");
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
