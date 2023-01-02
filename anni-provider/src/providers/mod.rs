pub use convention::CommonConventionProvider;
pub use drive::DriveProvider;
pub use multiple::MultipleProviders;
pub use no_cache::NoCacheStrictLocalProvider;
pub use proxy::ProxyBackend;
pub use strict::CommonStrictProvider;

mod convention;
pub mod drive;
mod multiple;
mod no_cache;
mod proxy;
mod strict;
