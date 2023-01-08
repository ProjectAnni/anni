#[cfg(feature = "convention")]
pub use convention::CommonConventionProvider;
#[cfg(feature = "drive")]
pub use drive::DriveProvider;
pub use multiple::MultipleProviders;
pub use no_cache::NoCacheStrictLocalProvider;
#[cfg(feature = "proxy")]
pub use proxy::ProxyBackend;
#[cfg(feature = "strict")]
pub use strict::CommonStrictProvider;

#[cfg(feature = "convention")]
mod convention;
#[cfg(feature = "drive")]
pub mod drive;
mod multiple;
mod no_cache;
#[cfg(feature = "proxy")]
mod proxy;
#[cfg(feature = "strict")]
mod strict;
