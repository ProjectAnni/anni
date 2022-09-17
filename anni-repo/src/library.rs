use crate::models::AnniDate;
use once_cell::sync::Lazy;
use regex::Regex;
use std::path::Path;
use std::str::FromStr;
use thiserror::Error;

pub fn file_name<P: AsRef<Path>>(path: P) -> std::io::Result<String> {
    let path = if path.as_ref().is_absolute() {
        path.as_ref().to_path_buf()
    } else {
        path.as_ref().canonicalize()?
    };
    Ok(path
        .file_name()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "No filename found"))?
        .to_str()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid UTF-8 path"))?
        .to_owned())
}

pub(crate) fn file_stem<P: AsRef<Path>>(path: P) -> std::io::Result<String> {
    let path = if path.as_ref().is_absolute() {
        path.as_ref().to_path_buf()
    } else {
        path.as_ref().canonicalize()?
    };
    Ok(path
        .file_stem()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "No filename found"))?
        .to_str()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid UTF-8 path"))?
        .to_owned())
}

#[derive(Error, Debug)]
pub enum InfoParseError {
    #[error("no match found: {0}")]
    NotMatch(String),
    #[error("no capture group matched")]
    NoCaptureGroup,
    #[error("invalid datetime")]
    InvalidDateTime,
}

static ALBUM_INFO: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\[(\d{4}|\d{2})-?(\d{2})-?(\d{2})]\[([^]]+)] (.+?)(?:【([^】]+)】)?(?: \[(\d+) Discs])?$").unwrap()
});
static DISC_INFO: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\[([^]]+)] (.+) \[Disc (\d+)]$").unwrap());

// catalog, title, disc_id
pub fn disc_info(path: &str) -> Result<(String, String, usize), InfoParseError> {
    let r = DISC_INFO
        .captures(path)
        .ok_or_else(|| InfoParseError::NotMatch(path.to_string()))?;
    if r.len() == 0 {
        return Err(InfoParseError::NoCaptureGroup);
    }

    Ok((
        r.get(1).unwrap().as_str().to_owned(),
        r.get(2).unwrap().as_str().to_owned(),
        usize::from_str(r.get(3).unwrap().as_str()).unwrap(),
    ))
}

pub struct AlbumInfo {
    pub release_date: AnniDate,
    pub catalog: String,
    pub title: String,
    pub edition: Option<String>,
    pub disc_count: usize,
}

impl FromStr for AlbumInfo {
    type Err = InfoParseError;

    fn from_str(path: &str) -> Result<Self, Self::Err> {
        let r = ALBUM_INFO
            .captures(path)
            .ok_or_else(|| InfoParseError::NotMatch(path.to_string()))?;
        if r.len() == 0 {
            return Err(InfoParseError::NoCaptureGroup);
        }

        Ok(AlbumInfo {
            release_date: AnniDate::from_parts(
                r.get(1).unwrap().as_str(),
                r.get(2).unwrap().as_str(),
                r.get(3).unwrap().as_str(),
            ),
            catalog: r.get(4).unwrap().as_str().replace('/', "／"),
            title: r.get(5).unwrap().as_str().replace('/', "／"),
            edition: r.get(6).map(|x| x.as_str().to_string()),
            disc_count: usize::from_str(r.get(7).map(|r| r.as_str()).unwrap_or("1")).unwrap(),
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_album_info() {
        let (date, catalog, title, edition, disc_count) =
            super::album_info("[220302][SMCL-753] 彩色硝子").unwrap();
        assert_eq!(date.to_string(), "2022-03-02");
        assert_eq!(catalog, "SMCL-753");
        assert_eq!(title, "彩色硝子");
        assert_eq!(edition, None);
        assert_eq!(disc_count, 1);

        let (date, catalog, title, edition, disc_count) =
            super::album_info("[2022-03-02][SMCL-753] 彩色硝子 [1 Discs]").unwrap();
        assert_eq!(date.to_string(), "2022-03-02");
        assert_eq!(catalog, "SMCL-753");
        assert_eq!(title, "彩色硝子");
        assert_eq!(edition, None);
        assert_eq!(disc_count, 1);

        let (date, catalog, title, edition, disc_count) =
            super::album_info("[2022-03-02][SMCL-753] 彩色硝子【Edition】").unwrap();
        assert_eq!(date.to_string(), "2022-03-02");
        assert_eq!(catalog, "SMCL-753");
        assert_eq!(title, "彩色硝子");
        assert_eq!(edition, Some("Edition".to_string()));
        assert_eq!(disc_count, 1);

        let (date, catalog, title, edition, disc_count) =
            super::album_info("[2022-03-02][SMCL-753] 彩色硝子【Edition】 [1 Discs]").unwrap();
        assert_eq!(date.to_string(), "2022-03-02");
        assert_eq!(catalog, "SMCL-753");
        assert_eq!(title, "彩色硝子");
        assert_eq!(edition, Some("Edition".to_string()));
        assert_eq!(edition, None);
        assert_eq!(disc_count, 1);
    }
}
