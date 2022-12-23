use crate::models::{AnniDate, DiscInfo};
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

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct AlbumFolderInfo {
    pub release_date: AnniDate,
    pub catalog: String,
    pub title: String,
    pub edition: Option<String>,
    pub disc_count: usize,
}

impl FromStr for AlbumFolderInfo {
    type Err = InfoParseError;

    fn from_str(path: &str) -> Result<Self, Self::Err> {
        let r = ALBUM_INFO
            .captures(path)
            .ok_or_else(|| InfoParseError::NotMatch(path.to_string()))?;
        if r.len() == 0 {
            return Err(InfoParseError::NoCaptureGroup);
        }

        Ok(AlbumFolderInfo {
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

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct DiscFolderInfo {
    pub info: DiscInfo,
    pub disc_id: usize,
}

impl FromStr for DiscFolderInfo {
    type Err = InfoParseError;

    fn from_str(path: &str) -> Result<Self, Self::Err> {
        let r = DISC_INFO
            .captures(path)
            .ok_or_else(|| InfoParseError::NotMatch(path.to_string()))?;
        if r.len() == 0 {
            return Err(InfoParseError::NoCaptureGroup);
        }

        Ok(DiscFolderInfo {
            info: DiscInfo::new(
                r.get(1).unwrap().as_str().replace('/', "／"),
                Some(r.get(2).unwrap().as_str().replace('/', "／")),
                None,
                None,
                Default::default(),
            ),
            disc_id: usize::from_str(r.get(3).unwrap().as_str()).unwrap(),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::library::{AlbumFolderInfo, DiscFolderInfo};
    use crate::models::{AnniDate, DiscInfo};
    use std::str::FromStr;

    fn album_info(title: &str) -> (AnniDate, String, String, Option<String>, usize) {
        let info = super::AlbumFolderInfo::from_str(title).unwrap();
        (
            info.release_date,
            info.catalog,
            info.title,
            info.edition,
            info.disc_count,
        )
    }

    #[test]
    fn test_album_info() {
        let (date, catalog, title, edition, disc_count) = album_info("[220302][SMCL-753] 彩色硝子");
        assert_eq!(date.to_string(), "2022-03-02");
        assert_eq!(catalog, "SMCL-753");
        assert_eq!(title, "彩色硝子");
        assert_eq!(edition, None);
        assert_eq!(disc_count, 1);

        let (date, catalog, title, edition, disc_count) =
            album_info("[2022-03-02][SMCL-753] 彩色硝子 [1 Discs]");
        assert_eq!(date.to_string(), "2022-03-02");
        assert_eq!(catalog, "SMCL-753");
        assert_eq!(title, "彩色硝子");
        assert_eq!(edition, None);
        assert_eq!(disc_count, 1);

        let (date, catalog, title, edition, disc_count) =
            album_info("[2022-03-02][SMCL-753] 彩色硝子【Edition】");
        assert_eq!(date.to_string(), "2022-03-02");
        assert_eq!(catalog, "SMCL-753");
        assert_eq!(title, "彩色硝子");
        assert_eq!(edition, Some("Edition".to_string()));
        assert_eq!(disc_count, 1);

        let (date, catalog, title, edition, disc_count) =
            album_info("[2022-03-02][SMCL-753] 彩色硝子【Edition】 [1 Discs]");
        assert_eq!(date.to_string(), "2022-03-02");
        assert_eq!(catalog, "SMCL-753");
        assert_eq!(title, "彩色硝子");
        assert_eq!(edition, Some("Edition".to_string()));
        assert_eq!(disc_count, 1);

        assert_eq!(
            AlbumFolderInfo::from_str("[200102][CATA-001] TITLE").unwrap(),
            AlbumFolderInfo {
                release_date: AnniDate::from_parts("2020", "01", "02"),
                catalog: "CATA-001".to_string(),
                title: "TITLE".to_string(),
                edition: None,
                disc_count: 1
            }
        );
        assert_eq!(
            AlbumFolderInfo::from_str("[200102][CATA-001] TITLE [2 Discs").unwrap(),
            AlbumFolderInfo {
                release_date: AnniDate::from_parts("2020", "01", "02"),
                catalog: "CATA-001".to_string(),
                title: "TITLE [2 Discs".to_string(),
                edition: None,
                disc_count: 1,
            }
        );
        assert_eq!(
            AlbumFolderInfo::from_str("[200102][CATA-001] TITLE [2 Discs]").unwrap(),
            AlbumFolderInfo {
                release_date: AnniDate::from_parts("2020", "01", "02"),
                catalog: "CATA-001".to_string(),
                title: "TITLE".to_string(),
                edition: None,
                disc_count: 2,
            }
        );
    }

    #[test]
    fn test_disc_info() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            DiscFolderInfo::from_str("[CATA-001] TITLE [Disc 1]")?,
            DiscFolderInfo {
                info: DiscInfo::new(
                    "CATA-001".to_string(),
                    Some("TITLE".to_string()),
                    None,
                    None,
                    vec![]
                ),
                disc_id: 1,
            }
        );
        Ok(())
    }
}
