#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("invalid {target} toml: {err:?}\n{input}")]
    TomlParseError {
        target: &'static str,
        input: String,
        err: toml::de::Error,
    },

    #[error("invalid track type: {0}")]
    InvalidTrackType(String),

    #[error("invalid date: {0}")]
    InvalidDate(String),

    #[error("invalid tag type: {0}")]
    InvalidTagType(String),

    #[error(transparent)]
    IOError(#[from] std::io::Error),

    #[error("multiple errors detected: {0:#?}")]
    MultipleErrors(Vec<Error>),
}

pub type MetadataResult<T> = Result<T, Error>;
