use anni_repo::Datetime;
use anni_repo::album::{Disc, Track};
use anni_flac::Stream;
use std::error::Error;

pub(crate) fn stream_to_track(stream: &Stream) -> Track {
    let comment = stream.comments().unwrap();
    Track::new(comment["TITLE"].value(), Some(comment["ARTIST"].value()), None)
}
