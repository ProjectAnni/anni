use std::{cmp::Ordering, fmt, num::NonZeroU32, str::FromStr};

use thiserror::Error;
use url::Url;

const PREFERRED_APPLE_ARTWORK_SIZE: u16 = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoverCandidateState {
    Discovered,
    Queued,
    Fetching,
    Verified,
    Rejected,
}

impl CoverCandidateState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Discovered => "discovered",
            Self::Queued => "queued",
            Self::Fetching => "fetching",
            Self::Verified => "verified",
            Self::Rejected => "rejected",
        }
    }

    pub const fn can_transition_to(self, next: Self) -> bool {
        self as u8 == next as u8
            || matches!(
                (self, next),
                (Self::Discovered, Self::Queued | Self::Rejected)
                    | (Self::Queued, Self::Fetching | Self::Rejected)
                    | (
                        Self::Fetching,
                        Self::Verified | Self::Queued | Self::Rejected
                    )
                    | (Self::Rejected, Self::Queued)
            )
    }
}

impl fmt::Display for CoverCandidateState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for CoverCandidateState {
    type Err = UnknownCoverCandidateState;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "discovered" => Ok(Self::Discovered),
            "queued" => Ok(Self::Queued),
            "fetching" => Ok(Self::Fetching),
            "verified" => Ok(Self::Verified),
            "rejected" => Ok(Self::Rejected),
            _ => Err(UnknownCoverCandidateState(value.to_owned())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("unknown cover candidate state: {0}")]
pub struct UnknownCoverCandidateState(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoverQuality {
    width: NonZeroU32,
    height: NonZeroU32,
    byte_length: u64,
}

impl CoverQuality {
    pub const fn new(width: NonZeroU32, height: NonZeroU32, byte_length: u64) -> Self {
        Self {
            width,
            height,
            byte_length,
        }
    }

    pub const fn width(self) -> NonZeroU32 {
        self.width
    }

    pub const fn height(self) -> NonZeroU32 {
        self.height
    }

    pub const fn byte_length(self) -> u64 {
        self.byte_length
    }

    pub fn pixel_area(self) -> u64 {
        u64::from(self.width.get()) * u64::from(self.height.get())
    }

    pub const fn shortest_side(self) -> u32 {
        if self.width.get() < self.height.get() {
            self.width.get()
        } else {
            self.height.get()
        }
    }
}

impl Ord for CoverQuality {
    fn cmp(&self, other: &Self) -> Ordering {
        self.shortest_side()
            .cmp(&other.shortest_side())
            .then_with(|| self.pixel_area().cmp(&other.pixel_area()))
            .then_with(|| self.byte_length.cmp(&other.byte_length))
    }
}

impl PartialOrd for CoverQuality {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Validate a remote cover URL and remove Amazon's filename compression block.
///
/// Query parameters are preserved because they can contain signed access
/// credentials rather than image transforms. Non-Amazon URLs are returned
/// byte-for-byte after validation.
pub fn canonicalize_cover_url(value: &str) -> Result<String, CoverUrlError> {
    let url = validate_remote_url(value)?;
    if !is_amazon_image_host(url.host_str().expect("validated URL has host")) {
        return Ok(value.to_owned());
    }

    canonicalize_amazon_url(url)
}

/// Derive an original-quality request URL for a known Amazon image CDN.
/// Unknown hosts are rejected so source-specific rules cannot be applied to
/// an unrelated website.
pub fn preferred_amazon_artwork_url(value: &str) -> Result<String, CoverUrlError> {
    let url = validate_remote_url(value)?;
    if !is_amazon_image_host(url.host_str().expect("validated URL has host")) {
        return Err(CoverUrlError::NotAmazonArtworkHost);
    }
    canonicalize_amazon_url(url)
}

fn canonicalize_amazon_url(mut url: Url) -> Result<String, CoverUrlError> {
    if let Some(path) = amazon_original_path(url.path()) {
        url.set_path(&path);
    }
    url.set_fragment(None);
    Ok(url.into())
}

/// Expand the width/height placeholders from Apple Music artwork responses.
/// The server still verifies the downloaded dimensions before accepting it.
pub fn preferred_apple_artwork_url(template: &str) -> Result<String, CoverUrlError> {
    if !template.contains("{w}") || !template.contains("{h}") {
        return Err(CoverUrlError::MissingAppleSizeTemplate);
    }
    let expanded = template
        .replace("{w}", &PREFERRED_APPLE_ARTWORK_SIZE.to_string())
        .replace("{h}", &PREFERRED_APPLE_ARTWORK_SIZE.to_string())
        .replace("{f}", "jpg");
    let url = validate_remote_url(&expanded)?;
    let host = url.host_str().expect("validated URL has host");
    if host != "mzstatic.com" && !host.ends_with(".mzstatic.com") {
        return Err(CoverUrlError::NotAppleArtworkHost);
    }
    Ok(expanded)
}

fn validate_remote_url(value: &str) -> Result<Url, CoverUrlError> {
    let url = Url::parse(value).map_err(CoverUrlError::InvalidUrl)?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(CoverUrlError::UnsupportedScheme);
    }
    if url.host_str().is_none() {
        return Err(CoverUrlError::MissingHost);
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(CoverUrlError::EmbeddedCredentials);
    }
    Ok(url)
}

fn is_amazon_image_host(host: &str) -> bool {
    host == "m.media-amazon.com"
        || host.ends_with(".media-amazon.com")
        || host == "ssl-images-amazon.com"
        || host.ends_with(".ssl-images-amazon.com")
        || host == "images.amazon.com"
        || host.ends_with(".images-amazon.com")
}

fn amazon_original_path(path: &str) -> Option<String> {
    let filename_start = path.rfind('/').map_or(0, |index| index + 1);
    let filename = &path[filename_start..];
    let parameter_start = filename.rfind("._")?;
    let after_start = &filename[parameter_start + 2..];
    let parameter_end = after_start.find("_.")?;
    let remove_end = filename_start + parameter_start + 2 + parameter_end + 1;
    let remove_start = filename_start + parameter_start;
    let mut result = path.to_owned();
    result.replace_range(remove_start..remove_end, "");
    Some(result)
}

#[derive(Debug, Error)]
pub enum CoverUrlError {
    #[error("invalid cover URL: {0}")]
    InvalidUrl(#[source] url::ParseError),
    #[error("cover URL must use HTTP or HTTPS")]
    UnsupportedScheme,
    #[error("cover URL must include a host")]
    MissingHost,
    #[error("cover URL cannot contain embedded credentials")]
    EmbeddedCredentials,
    #[error("Apple artwork URL must include both {{w}} and {{h}} placeholders")]
    MissingAppleSizeTemplate,
    #[error("Apple artwork URL must use an mzstatic.com host")]
    NotAppleArtworkHost,
    #[error("Amazon artwork URL must use a known Amazon image CDN host")]
    NotAmazonArtworkHost,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amazon_compression_block_is_removed() {
        let source = "https://m.media-amazon.com/images/I/81abc._AC_SL1500_.jpg?x=1";
        assert_eq!(
            canonicalize_cover_url(source).unwrap(),
            "https://m.media-amazon.com/images/I/81abc.jpg?x=1"
        );
        assert!(matches!(
            preferred_amazon_artwork_url("https://artist.example/cover._SL100_.jpg"),
            Err(CoverUrlError::NotAmazonArtworkHost)
        ));
    }

    #[test]
    fn validated_non_amazon_url_is_not_rewritten() {
        let source = "https://artist.example/ジャケット（初回）.png?size=full";
        assert_eq!(canonicalize_cover_url(source).unwrap(), source);
        assert!(matches!(
            canonicalize_cover_url("file:///tmp/cover.jpg"),
            Err(CoverUrlError::UnsupportedScheme)
        ));
    }

    #[test]
    fn apple_artwork_template_requests_preferred_size() {
        let template = "https://is1-ssl.mzstatic.com/image/thumb/Music/a/b/c/{w}x{h}bb.{f}";
        assert_eq!(
            preferred_apple_artwork_url(template).unwrap(),
            "https://is1-ssl.mzstatic.com/image/thumb/Music/a/b/c/10000x10000bb.jpg"
        );
    }

    #[test]
    fn cover_quality_prefers_shortest_side_then_pixels_and_bytes() {
        let small = CoverQuality::new(
            NonZeroU32::new(1_000).unwrap(),
            NonZeroU32::new(1_000).unwrap(),
            2_000_000,
        );
        let large = CoverQuality::new(
            NonZeroU32::new(2_000).unwrap(),
            NonZeroU32::new(2_000).unwrap(),
            1_000_000,
        );
        assert!(large > small);

        let banner = CoverQuality::new(
            NonZeroU32::new(20_000).unwrap(),
            NonZeroU32::new(100).unwrap(),
            3_000_000,
        );
        assert!(small > banner);
    }

    #[test]
    fn cover_candidate_fetch_lifecycle_is_recoverable() {
        assert!(CoverCandidateState::Discovered.can_transition_to(CoverCandidateState::Queued));
        assert!(CoverCandidateState::Queued.can_transition_to(CoverCandidateState::Fetching));
        assert!(CoverCandidateState::Fetching.can_transition_to(CoverCandidateState::Queued));
        assert!(CoverCandidateState::Fetching.can_transition_to(CoverCandidateState::Verified));
        assert!(!CoverCandidateState::Verified.can_transition_to(CoverCandidateState::Queued));
    }
}
