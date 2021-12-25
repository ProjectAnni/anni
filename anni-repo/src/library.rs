use regex::Regex;
use std::path::Path;
use std::str::FromStr;
use thiserror::Error;
use toml::value::DatetimeParseError;
use crate::models::AnniDate;

pub fn file_name<P: AsRef<Path>>(path: P) -> std::io::Result<String> {
    let path = if path.as_ref().is_absolute() {
        path.as_ref().to_path_buf()
    } else {
        path.as_ref().canonicalize()?
    };
    Ok(path
        .file_name()
        .ok_or_else(|| std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "No filename found",
        ))?
        .to_str()
        .ok_or_else(|| std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Invalid UTF-8 path",
        ))?
        .to_owned())
}

#[derive(Error, Debug)]
pub enum InfoParseError {
    #[error("no match found")]
    NotMatch,
    #[error("no capture group matched")]
    NoCaptureGroup,
    #[error("invalid datetime")]
    InvalidDateTime(#[from] DatetimeParseError),
}

// catalog, title, disc_id
pub fn disc_info(path: &str) -> Result<(String, String, usize), InfoParseError> {
    let r = Regex::new(r"^\[([^]]+)] (.+) \[Disc (\d+)]$").unwrap();
    let r = r.captures(path).ok_or(InfoParseError::NotMatch)?;
    if r.len() == 0 {
        return Err(InfoParseError::NoCaptureGroup);
    }

    Ok((
        r.get(1).unwrap().as_str().to_owned(),
        r.get(2).unwrap().as_str().to_owned(),
        usize::from_str(r.get(3).unwrap().as_str()).unwrap(),
    ))
}

// Date, catalog, title, disc_count
pub fn album_info(path: &str) -> Result<(AnniDate, String, String, usize), InfoParseError> {
    let r = Regex::new(r"^\[(\d{2}|\d{4})-?(\d{2})-?(\d{2})]\[([^]]+)] (.+?)(?: \[(\d+) Discs])?$").unwrap();
    let r = r.captures(path).ok_or(InfoParseError::NotMatch)?;
    if r.len() == 0 {
        return Err(InfoParseError::NoCaptureGroup);
    }

    Ok((
        AnniDate::from_parts(
            r.get(1).unwrap().as_str(),
            r.get(2).unwrap().as_str(),
            r.get(3).unwrap().as_str(),
        ),
        r.get(4).unwrap().as_str().replace('/', "／"),
        r.get(5).unwrap().as_str().replace('/', "／"),
        usize::from_str(r.get(6).map(|r| r.as_str()).unwrap_or("1")).unwrap(),
    ))
}
