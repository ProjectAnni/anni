//! Safe remote cover acquisition and content-addressed storage.
//!
//! Network access ends in this crate. Annim persists candidates and leases;
//! immutable ingest plans only see the verified local asset digest and key.

use std::{
    collections::HashSet,
    fmt,
    fs::{self, File},
    io::Read,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    path::{Path, PathBuf},
    time::Duration,
};

use anni_catalog::{cover_asset_storage_key, CoverMediaType};
use anni_ingest::Digest;
use futures_util::StreamExt;
use image::{ImageFormat, ImageReader, Limits};
use reqwest::{
    header::{ACCEPT, LOCATION},
    redirect::Policy,
    Response, StatusCode,
};
use sha2::{Digest as ShaDigest, Sha256};
use thiserror::Error;
use tokio::io::AsyncWriteExt;
use url::Url;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoverFetchPolicy {
    pub max_redirects: usize,
    pub max_encoded_bytes: u64,
    pub max_dimension: u32,
    pub max_pixels: u64,
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
}

impl Default for CoverFetchPolicy {
    fn default() -> Self {
        Self {
            max_redirects: 5,
            max_encoded_bytes: 100 * 1024 * 1024,
            max_dimension: 20_000,
            max_pixels: 120_000_000,
            connect_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(45),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AssetStore {
    root: PathBuf,
    incoming: PathBuf,
}

impl AssetStore {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, CoverWorkerError> {
        let root = fs::canonicalize(root)?;
        if !root.is_dir() {
            return Err(CoverWorkerError::AssetRootNotDirectory { path: root });
        }
        let incoming = root.join(".incoming");
        fs::create_dir_all(&incoming)?;
        let incoming = fs::canonicalize(incoming)?;
        if !incoming.starts_with(&root) {
            return Err(CoverWorkerError::AssetRootNotDirectory { path: root });
        }
        Ok(Self { root, incoming })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn pending_path(&self) -> PathBuf {
        self.incoming.join(format!("{}.partial", Uuid::new_v4()))
    }

    fn target_path(&self, storage_key: &str) -> PathBuf {
        storage_key
            .split('/')
            .fold(self.root.clone(), |path, component| path.join(component))
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct VerifiedCoverDownload {
    digest: Digest,
    storage_key: String,
    media_type: CoverMediaType,
    width: u32,
    height: u32,
    byte_length: u64,
    effective_url: String,
}

impl VerifiedCoverDownload {
    pub const fn digest(&self) -> Digest {
        self.digest
    }

    pub fn storage_key(&self) -> &str {
        &self.storage_key
    }

    pub const fn media_type(&self) -> CoverMediaType {
        self.media_type
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }

    /// This URL may contain signed query parameters. It is intended only for
    /// persistence in the private candidate record, never ordinary logging.
    pub fn effective_url(&self) -> &str {
        &self.effective_url
    }
}

impl fmt::Debug for VerifiedCoverDownload {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedCoverDownload")
            .field("digest", &self.digest)
            .field("storage_key", &self.storage_key)
            .field("media_type", &self.media_type)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("byte_length", &self.byte_length)
            .field("effective_origin", &origin_label(&self.effective_url))
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct CoverDownloader {
    policy: CoverFetchPolicy,
}

impl CoverDownloader {
    pub const fn new(policy: CoverFetchPolicy) -> Self {
        Self { policy }
    }

    pub const fn policy(&self) -> CoverFetchPolicy {
        self.policy
    }

    pub async fn download(
        &self,
        requested_url: &str,
        store: &AssetStore,
    ) -> Result<VerifiedCoverDownload, CoverWorkerError> {
        let mut url = validate_remote_url(requested_url)?;
        let mut redirects = 0_usize;
        let response = loop {
            let response = self.request_once(&url).await?;
            if !is_followed_redirect(response.status()) {
                break response;
            }
            if redirects >= self.policy.max_redirects {
                return Err(CoverWorkerError::TooManyRedirects);
            }
            url = redirect_target(&url, &response)?;
            redirects += 1;
            // The next request will resolve and pin the new host again.
        };

        if !response.status().is_success() {
            return Err(CoverWorkerError::HttpStatus {
                status: response.status().as_u16(),
                origin: origin_label(response.url().as_str()),
            });
        }
        if response
            .content_length()
            .is_some_and(|length| length > self.policy.max_encoded_bytes)
        {
            return Err(CoverWorkerError::BodyTooLarge {
                limit: self.policy.max_encoded_bytes,
            });
        }

        let effective_url = response.url().to_string();
        let pending_path = store.pending_path();
        let mut pending = PendingFile::new(pending_path.clone());
        let mut output = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&pending_path)
            .await?;
        let mut hasher = Sha256::new();
        let mut byte_length = 0_u64;
        let mut body = response.bytes_stream();
        while let Some(chunk) = body.next().await {
            let chunk = chunk.map_err(|source| CoverWorkerError::Request {
                origin: origin_label(&effective_url),
                source: source.without_url(),
            })?;
            byte_length = byte_length.checked_add(chunk.len() as u64).ok_or(
                CoverWorkerError::BodyTooLarge {
                    limit: self.policy.max_encoded_bytes,
                },
            )?;
            if byte_length > self.policy.max_encoded_bytes {
                return Err(CoverWorkerError::BodyTooLarge {
                    limit: self.policy.max_encoded_bytes,
                });
            }
            hasher.update(&chunk);
            output.write_all(&chunk).await?;
        }
        output.flush().await?;
        output.sync_all().await?;
        drop(output);

        if byte_length == 0 {
            return Err(CoverWorkerError::EmptyBody);
        }
        let digest = Digest::new(hasher.finalize().into());
        let policy = self.policy;
        let store = store.clone();
        let verified = tokio::task::spawn_blocking(move || {
            verify_and_commit(&store, &pending_path, digest, byte_length, policy)
        })
        .await
        .map_err(|_| CoverWorkerError::VerifierPanicked)??;
        // The object is a hard link to the fsynced partial; removing only the
        // worker-owned name cannot remove the content-addressed object.
        if fs::remove_file(&pending.path).is_ok() {
            pending.committed = true;
        }

        Ok(VerifiedCoverDownload {
            digest,
            storage_key: verified.storage_key,
            media_type: verified.media_type,
            width: verified.width,
            height: verified.height,
            byte_length,
            effective_url,
        })
    }

    async fn request_once(&self, url: &Url) -> Result<Response, CoverWorkerError> {
        validate_url(url)?;
        let host = url
            .host_str()
            .ok_or(CoverWorkerError::MissingHost)?
            .to_owned();
        let port = url.port_or_known_default().unwrap_or(443);
        let addrs = resolve_public_addrs(&host, port).await?;
        let client = reqwest::Client::builder()
            .redirect(Policy::none())
            .no_proxy()
            .referer(false)
            .connect_timeout(self.policy.connect_timeout)
            .timeout(self.policy.request_timeout)
            .user_agent("ProjectAnni-CoverWorker/0.1")
            .resolve_to_addrs(&host, &addrs)
            .build()
            .map_err(|source| CoverWorkerError::Client(source.without_url()))?;
        client
            .get(url.clone())
            .header(ACCEPT, "image/webp,image/png,image/jpeg;q=0.9")
            .send()
            .await
            .map_err(|source| CoverWorkerError::Request {
                origin: origin_label(url.as_str()),
                source: source.without_url(),
            })
    }
}

impl Default for CoverDownloader {
    fn default() -> Self {
        Self::new(CoverFetchPolicy::default())
    }
}

fn is_followed_redirect(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::MOVED_PERMANENTLY
            | StatusCode::FOUND
            | StatusCode::SEE_OTHER
            | StatusCode::TEMPORARY_REDIRECT
            | StatusCode::PERMANENT_REDIRECT
    )
}

fn redirect_target(current: &Url, response: &Response) -> Result<Url, CoverWorkerError> {
    let location = response
        .headers()
        .get(LOCATION)
        .ok_or(CoverWorkerError::RedirectWithoutLocation)?
        .to_str()
        .map_err(|_| CoverWorkerError::InvalidRedirectLocation)?;
    let target = current
        .join(location)
        .map_err(|_| CoverWorkerError::InvalidRedirectLocation)?;
    validate_url(&target)?;
    Ok(target)
}

fn validate_remote_url(value: &str) -> Result<Url, CoverWorkerError> {
    let url = Url::parse(value).map_err(CoverWorkerError::InvalidUrl)?;
    validate_url(&url)?;
    Ok(url)
}

fn validate_url(url: &Url) -> Result<(), CoverWorkerError> {
    if url.scheme() != "https" {
        return Err(CoverWorkerError::HttpsRequired);
    }
    if url.host_str().is_none() {
        return Err(CoverWorkerError::MissingHost);
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(CoverWorkerError::EmbeddedCredentials);
    }
    Ok(())
}

async fn resolve_public_addrs(host: &str, port: u16) -> Result<Vec<SocketAddr>, CoverWorkerError> {
    let resolved = tokio::net::lookup_host((host, port))
        .await
        .map_err(|source| CoverWorkerError::Resolve {
            host: host.to_owned(),
            source,
        })?;
    let mut unique = HashSet::new();
    for address in resolved {
        if !is_public_ip(address.ip()) {
            return Err(CoverWorkerError::ForbiddenAddress {
                host: host.to_owned(),
                address: address.ip(),
            });
        }
        unique.insert(address);
    }
    if unique.is_empty() {
        return Err(CoverWorkerError::NoAddresses {
            host: host.to_owned(),
        });
    }
    Ok(unique.into_iter().collect())
}

fn is_public_ip(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => is_public_ipv4(address),
        IpAddr::V6(address) => is_public_ipv6(address),
    }
}

fn is_public_ipv4(address: Ipv4Addr) -> bool {
    let [a, b, c, _] = address.octets();
    !(address.is_private()
        || address.is_loopback()
        || address.is_link_local()
        || address.is_multicast()
        || address.is_broadcast()
        || address.is_documentation()
        || address.is_unspecified()
        || a == 0
        || (a == 100 && (64..=127).contains(&b))
        || (a == 192 && b == 0 && c == 0)
        || (a == 192 && b == 88 && c == 99)
        || (a == 198 && matches!(b, 18 | 19))
        || a >= 240)
}

fn is_public_ipv6(address: Ipv6Addr) -> bool {
    if let Some(mapped) = address.to_ipv4_mapped() {
        return is_public_ipv4(mapped);
    }
    let first = address.segments()[0];
    !address.is_unspecified()
        && !address.is_loopback()
        && !address.is_multicast()
        && !address.is_unique_local()
        && !address.is_unicast_link_local()
        && !(first & 0xffc0 == 0xfec0)
        && address.segments()[0..2] != [0x2001, 0x0db8]
        // Be conservative: currently allocated global unicast space.
        && first & 0xe000 == 0x2000
}

#[derive(Debug)]
struct CommittedAsset {
    storage_key: String,
    media_type: CoverMediaType,
    width: u32,
    height: u32,
}

fn verify_and_commit(
    store: &AssetStore,
    pending_path: &Path,
    digest: Digest,
    byte_length: u64,
    policy: CoverFetchPolicy,
) -> Result<CommittedAsset, CoverWorkerError> {
    let (media_type, width, height) = inspect_image(pending_path, policy)?;
    let storage_key = cover_asset_storage_key(digest.as_bytes(), media_type);
    let target = store.target_path(&storage_key);
    let requested_parent = target
        .parent()
        .ok_or_else(|| CoverWorkerError::InvalidStorageKey(storage_key.clone()))?;
    fs::create_dir_all(requested_parent)?;
    let parent = fs::canonicalize(requested_parent)?;
    if !parent.starts_with(&store.root) {
        return Err(CoverWorkerError::AssetPathEscapesRoot { path: parent });
    }
    let file_name = target
        .file_name()
        .ok_or_else(|| CoverWorkerError::InvalidStorageKey(storage_key.clone()))?;
    let target = parent.join(file_name);
    match fs::hard_link(pending_path, &target) {
        Ok(()) => File::open(parent)?.sync_all()?,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            if fs::symlink_metadata(&target)?.file_type().is_symlink() {
                return Err(CoverWorkerError::AssetPathEscapesRoot { path: target });
            }
            let (actual_digest, actual_length) = digest_file(&target)?;
            if actual_digest != digest || actual_length != byte_length {
                return Err(CoverWorkerError::DigestCollision { storage_key });
            }
        }
        Err(error) => return Err(error.into()),
    }
    Ok(CommittedAsset {
        storage_key,
        media_type,
        width,
        height,
    })
}

fn inspect_image(
    path: &Path,
    policy: CoverFetchPolicy,
) -> Result<(CoverMediaType, u32, u32), CoverWorkerError> {
    let reader = ImageReader::open(path)?.with_guessed_format()?;
    let format = reader
        .format()
        .ok_or(CoverWorkerError::UnsupportedImageFormat)?;
    let media_type = match format {
        ImageFormat::Jpeg => CoverMediaType::Jpeg,
        ImageFormat::Png => CoverMediaType::Png,
        ImageFormat::WebP => CoverMediaType::Webp,
        _ => return Err(CoverWorkerError::UnsupportedImageFormat),
    };
    let (width, height) = reader.into_dimensions()?;
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or(CoverWorkerError::ImageTooLarge)?;
    if width == 0
        || height == 0
        || width > policy.max_dimension
        || height > policy.max_dimension
        || pixels > policy.max_pixels
    {
        return Err(CoverWorkerError::ImageTooLarge);
    }

    let mut limits = Limits::default();
    limits.max_image_width = Some(policy.max_dimension);
    limits.max_image_height = Some(policy.max_dimension);
    limits.max_alloc = policy.max_pixels.checked_mul(4);
    let mut reader = ImageReader::open(path)?.with_guessed_format()?;
    reader.limits(limits);
    reader.decode()?;
    Ok((media_type, width, height))
}

fn digest_file(path: &Path) -> Result<(Digest, u64), CoverWorkerError> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut length = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        length = length
            .checked_add(read as u64)
            .ok_or(CoverWorkerError::ImageTooLarge)?;
    }
    Ok((Digest::new(hasher.finalize().into()), length))
}

fn origin_label(value: &str) -> String {
    Url::parse(value)
        .ok()
        .and_then(|url| {
            url.host_str()
                .map(|host| format!("{}://{host}", url.scheme()))
        })
        .unwrap_or_else(|| "remote origin".to_owned())
}

struct PendingFile {
    path: PathBuf,
    committed: bool,
}

impl PendingFile {
    const fn new(path: PathBuf) -> Self {
        Self {
            path,
            committed: false,
        }
    }
}

impl Drop for PendingFile {
    fn drop(&mut self) {
        if !self.committed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[derive(Debug, Error)]
pub enum CoverWorkerError {
    #[error("invalid cover URL")]
    InvalidUrl(#[source] url::ParseError),
    #[error("cover downloads require HTTPS")]
    HttpsRequired,
    #[error("cover URL is missing a host")]
    MissingHost,
    #[error("cover URL cannot contain embedded credentials")]
    EmbeddedCredentials,
    #[error("could not resolve cover host {host}")]
    Resolve {
        host: String,
        #[source]
        source: std::io::Error,
    },
    #[error("cover host {host} resolved to forbidden address {address}")]
    ForbiddenAddress { host: String, address: IpAddr },
    #[error("cover host {host} resolved to no addresses")]
    NoAddresses { host: String },
    #[error("could not build cover HTTP client")]
    Client(#[from] reqwest::Error),
    #[error("cover request to {origin} failed")]
    Request {
        origin: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("cover response from {origin} returned HTTP {status}")]
    HttpStatus { status: u16, origin: String },
    #[error("cover redirect is missing Location")]
    RedirectWithoutLocation,
    #[error("cover redirect Location is invalid")]
    InvalidRedirectLocation,
    #[error("cover response exceeded redirect limit")]
    TooManyRedirects,
    #[error("cover response exceeded {limit} encoded bytes")]
    BodyTooLarge { limit: u64 },
    #[error("cover response body is empty")]
    EmptyBody,
    #[error("downloaded cover format is unsupported")]
    UnsupportedImageFormat,
    #[error("downloaded cover exceeds configured image limits")]
    ImageTooLarge,
    #[error("verified cover storage key is invalid: {0}")]
    InvalidStorageKey(String),
    #[error("content digest collision at {storage_key}")]
    DigestCollision { storage_key: String },
    #[error("asset repository root is not a directory: {path}")]
    AssetRootNotDirectory { path: PathBuf },
    #[error("asset path resolves outside configured root: {path}")]
    AssetPathEscapesRoot { path: PathBuf },
    #[error("cover verifier task terminated unexpectedly")]
    VerifierPanicked,
    #[error(transparent)]
    Image(#[from] image::ImageError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use image::{DynamicImage, ImageFormat};
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn network_policy_rejects_local_and_non_https_targets() {
        assert!(matches!(
            validate_remote_url("http://example.com/cover.jpg"),
            Err(CoverWorkerError::HttpsRequired)
        ));
        assert!(!is_public_ip("127.0.0.1".parse().unwrap()));
        assert!(!is_public_ip("169.254.169.254".parse().unwrap()));
        assert!(!is_public_ip("10.0.0.1".parse().unwrap()));
        assert!(!is_public_ip("::1".parse().unwrap()));
        assert!(!is_public_ip("fc00::1".parse().unwrap()));
        assert!(is_public_ip("1.1.1.1".parse().unwrap()));
        assert!(is_public_ip("2606:4700:4700::1111".parse().unwrap()));
    }

    #[test]
    fn verified_image_is_deduplicated_without_overwrite() {
        let root = tempdir().unwrap();
        let store = AssetStore::open(root.path()).unwrap();
        let image = DynamicImage::new_rgb8(8, 8);
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, ImageFormat::Png).unwrap();
        let bytes = bytes.into_inner();
        let digest = Digest::new(Sha256::digest(&bytes).into());
        let pending = store.pending_path();
        fs::write(&pending, &bytes).unwrap();

        let committed = verify_and_commit(
            &store,
            &pending,
            digest,
            bytes.len() as u64,
            CoverFetchPolicy::default(),
        )
        .unwrap();
        assert_eq!(committed.media_type, CoverMediaType::Png);
        assert_eq!((committed.width, committed.height), (8, 8));
        assert_eq!(
            fs::read(store.target_path(&committed.storage_key)).unwrap(),
            bytes
        );

        let second = store.pending_path();
        fs::write(&second, &bytes).unwrap();
        let reused = verify_and_commit(
            &store,
            &second,
            digest,
            bytes.len() as u64,
            CoverFetchPolicy::default(),
        )
        .unwrap();
        assert_eq!(reused.storage_key, committed.storage_key);
    }
}
