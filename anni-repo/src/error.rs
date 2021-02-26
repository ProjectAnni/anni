use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("invalid {target:?} toml: {err:?}")]
    TomlParseError {
        target: &'static str,
        err: toml::de::Error,
    },
    #[error("failed to initialize repository: {0}")]
    RepoInitError(anyhow::Error),
    #[error("failed to load album {album:?} in repository: {err:?}")]
    RepoAlbumLoadError {
        album: String,
        err: anyhow::Error,
    },
}