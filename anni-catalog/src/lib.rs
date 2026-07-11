//! Domain types for the server-side collection catalog.
//!
//! This crate deliberately separates desired/observed releases from files in
//! the ingest pipeline. Network sources produce observations; they never
//! overwrite canonical metadata or claim that audio has been collected.

mod collection;
mod cover;
mod source;

pub use collection::{AudioCodec, AudioProperties, CollectionState, QualityTier};
pub use cover::{canonicalize_cover_url, preferred_apple_artwork_url, CoverQuality, CoverUrlError};
pub use source::{AcquisitionSourceKind, CatalogSourceKind, CoverSourceKind};
