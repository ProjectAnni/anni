//! Domain types for the server-side collection catalog.
//!
//! This crate deliberately separates desired/observed releases from files in
//! the ingest pipeline. Network sources produce observations; they never
//! overwrite canonical metadata or claim that audio has been collected.

mod collection;
mod cover;
mod source;

pub use collection::{AudioCodec, AudioProperties, CollectionState, QualityTier, ReleaseKind};
pub use cover::{
    canonicalize_cover_url, cover_asset_storage_key, preferred_amazon_artwork_url,
    preferred_apple_artwork_url, CoverCandidateState, CoverMediaType, CoverQuality, CoverUrlError,
    UnknownCoverMediaType,
};
pub use source::{AcquisitionSourceKind, CatalogSourceKind, CoverSourceKind, SyncRunStatus};
