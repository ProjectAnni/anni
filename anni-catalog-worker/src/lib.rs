//! Safe background synchronization of external release catalogs.
//!
//! Annim owns durable runs, leases, and immutable observations. This crate
//! owns adapter pagination and retry orchestration. Adapter data is always
//! evidence: it never writes canonical Booklet metadata directly.

mod adapter;
mod runner;

pub use adapter::*;
pub use runner::*;
