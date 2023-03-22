use std::str::FromStr;

use anni_repo::prelude::Album;

macro_rules! format_test {
    ($path: expr) => {
        let mut album = Album::from_str(include_str!(concat!(
            "albums/format/",
            $path,
            "/formatted.toml"
        )))
        .unwrap();
        let formatted = album.format_to_string();
        let expected = include_str!(concat!("albums/format/", $path, "/formatted.toml"));
        assert_eq!(formatted, expected);
    };
}

#[test]
fn track_artist_to_disc_artist() {
    format_test!("track-artist-to-disc-artist");
}

#[test]
fn disc_artist_to_album_artist() {
    format_test!("disc-artist-to-album-artist");
}
