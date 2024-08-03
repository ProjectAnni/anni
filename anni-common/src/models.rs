use std::borrow::{Borrow, Cow};
use std::fmt::Display;
use std::num::NonZeroU8;
use std::str::{FromStr, Split};

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

    pub fn copied(&'a self) -> Self {
        Self::new(&self.album_id, self.disc_id, self.track_id)
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

impl<'a> Display for RawTrackIdentifier<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}/{}", self.album_id, self.disc_id, self.track_id)
    }
}

#[derive(Hash, PartialEq, Eq)]
pub struct TrackIdentifier {
    pub inner: RawTrackIdentifier<'static>,
}

impl<'a> Borrow<RawTrackIdentifier<'a>> for TrackIdentifier {
    fn borrow(&self) -> &RawTrackIdentifier<'a> {
        &self.inner
    }
}

impl FromStr for TrackIdentifier {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let read_u8 =
            |s: &mut Split<_>| s.next().ok_or(ParseError)?.parse().map_err(|_| ParseError);

        let mut sp = s.split('/');

        let album_id = sp.next().ok_or(ParseError)?;
        let disc_id = read_u8(&mut sp)?;
        let track_id = read_u8(&mut sp)?;

        Ok(RawTrackIdentifier::new(album_id, disc_id, track_id).to_owned())
    }
}

impl Display for TrackIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}

impl Clone for TrackIdentifier {
    fn clone(&self) -> Self {
        self.inner.to_owned()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ParseError;

impl Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "fail to parse track identifier")
    }
}

impl std::error::Error for ParseError {}
