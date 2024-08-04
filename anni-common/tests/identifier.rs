use std::num::NonZeroU8;

use anni_common::models::{ParseError, TrackIdentifier};

#[test]
fn parse() -> Result<(), ParseError> {
    let identifier = "65cf12dc-9717-4503-9901-848e8cd3ebff/1/8".parse::<TrackIdentifier>()?;

    assert_eq!(
        identifier.inner.album_id,
        "65cf12dc-9717-4503-9901-848e8cd3ebff"
    );
    assert_eq!(identifier.inner.disc_id, NonZeroU8::new(1).unwrap());
    assert_eq!(identifier.inner.track_id, NonZeroU8::new(8).unwrap());

    Ok(())
}
