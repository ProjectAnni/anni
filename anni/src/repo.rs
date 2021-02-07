use anni_repo::album::Track;
use anni_flac::Stream;

pub(crate) fn stream_to_track(stream: &Stream) -> Track {
    let comment = stream.comments().unwrap();
    Track::new(comment["TITLE"].value(), Some(comment["ARTIST"].value()), None)
}
