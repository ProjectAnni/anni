use std::borrow::{Borrow, Cow};
use std::num::NonZeroU8;

#[derive(Hash, PartialEq, Eq)]
pub struct RawTrackIdentifier<'album_id> {
    pub album_id: Cow<'album_id, str>,
    pub disc_id: NonZeroU8,
    pub track_id: NonZeroU8,
}

impl<'a> RawTrackIdentifier<'a> {
    pub fn new(album_id: &'a str, disc_id: NonZeroU8, track_id: NonZeroU8) -> Self {
        Self {
            album_id: Cow::Borrowed(album_id),
            disc_id,
            track_id,
        }
    }

    pub fn to_owned(&self) -> TrackIdentifier {
        TrackIdentifier {
            inner: RawTrackIdentifier {
                album_id: Cow::Owned(self.album_id.to_string()),
                disc_id: self.disc_id,
                track_id: self.track_id,
            },
        }
    }
}

impl<'a> Clone for RawTrackIdentifier<'a> {
    fn clone(&self) -> Self {
        Self {
            album_id: Cow::Owned(self.album_id.to_string()),
            disc_id: self.disc_id,
            track_id: self.track_id,
        }
    }
}

#[derive(Hash, PartialEq, Eq)]
pub struct TrackIdentifier {
    inner: RawTrackIdentifier<'static>,
}

impl<'a> Borrow<RawTrackIdentifier<'a>> for TrackIdentifier {
    fn borrow(&self) -> &RawTrackIdentifier<'a> {
        &self.inner
    }
}
