use anni_repo::Datetime;
use anni_repo::album::{Disc, Track};
use anni_flac::Stream;
use std::error::Error;

pub(crate) fn disc_to_repo_album(disc: Disc, title: &str, artist: &str, release: Datetime, catalog: &str) -> Result<anni_repo::Album, Box<dyn Error>> {
    let mut album = anni_repo::Album::new(title, artist, release, catalog);
    album.add_disc(disc);
    Ok(album)
}

pub(crate) fn stream_to_track(stream: &Stream) -> Track {
    let comment = stream.comments().unwrap();
    Track::new(comment["TITLE"].value(), Some(comment["ARTIST"].value()), None)
}
