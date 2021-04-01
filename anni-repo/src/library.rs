use crate::Datetime;
use regex::Regex;
use std::path::Path;
use std::str::FromStr;
use thiserror::Error;
use toml::value::DatetimeParseError;

pub fn file_name<P: AsRef<Path>>(path: P) -> std::io::Result<String> {
    let path = if path.as_ref().is_absolute() {
        path.as_ref().to_path_buf()
    } else {
        path.as_ref().canonicalize()?
    };
    Ok(path
        .file_name()
        .ok_or(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "No filename found",
        ))?
        .to_str()
        .ok_or(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Invalid UTF-8 path",
        ))?
        .to_owned())
}

pub fn parts_to_date(y: &str, m: &str, d: &str) -> Result<Datetime, DatetimeParseError> {
    // yyyy-mm-dd
    let mut date = String::with_capacity(10);
    // for yymmdd
    if y.len() == 2 {
        date += if u8::from_str(y).unwrap() > 30 {
            "19"
        } else {
            "20"
        }
    }
    date += y;
    date += "-";
    date += m;
    date += "-";
    date += d;
    Ok(Datetime::from_str(&date)?)
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

// Date, catalog, title
pub fn album_info(path: &str) -> Result<(Datetime, String, String), InfoParseError> {
    let r = Regex::new(r"^\[(\d{2}|\d{4})-?(\d{2})-?(\d{2})]\[([^]]+)] (.+)$").unwrap();
    let r = r.captures(path).ok_or(InfoParseError::NotMatch)?;
    if r.len() == 0 {
        return Err(InfoParseError::NoCaptureGroup);
    }

    Ok((
        parts_to_date(
            r.get(1).unwrap().as_str(),
            r.get(2).unwrap().as_str(),
            r.get(3).unwrap().as_str(),
        )?,
        r.get(4).unwrap().as_str().to_owned(),
        r.get(5).unwrap().as_str().to_owned(),
    ))
}
