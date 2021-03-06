use async_trait::async_trait;
use tokio::io::AsyncRead;
use regex::Regex;

/// Backend is a common trait for anni backends.
/// It provides functions to update albums, and read from an initialized backend.
#[async_trait]
pub trait Backend {
    /// Backend provided error type.
    type Err;

    /// Cache indicator for remote file systems.
    fn need_cache() -> bool;

    /// Update album information provided by backend.
    /// Backends usually need to save a map between catalog and path, so this method is &mut.
    async fn update_albums(&mut self) -> Result<Vec<&str>, Self::Err>;

    /// Returns a reader implements AsyncRead for content reading
    /// Since backend does not know which file to read, both track_id and track_name are necessary.
    async fn get_audio(&self, catalog: &str, track_id: u8, track_name: &str) -> Result<Box<dyn AsyncRead>, Self::Err>;
}

lazy_static::lazy_static! {
    static ref ALBUM_REGEX: Regex = Regex::new(r"^\[(?:\d{2}|\d{4})-?\d{2}-?\d{2}]\[([^]]+)] .+$").unwrap();
}

pub(crate) fn extract_catalog<S: AsRef<str>>(name: S) -> Option<String> {
    ALBUM_REGEX.captures(name.as_ref()).map(|r| r.get(1).unwrap().as_str().to_owned())
}

#[test]
fn test_extract_catalog() {
    assert_eq!(extract_catalog("[210306][CATA-LOG] Title"), Some("CATA-LOG".to_owned()));
    assert_eq!(extract_catalog("233"), None);
}