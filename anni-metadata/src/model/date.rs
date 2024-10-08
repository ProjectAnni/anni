use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{
    fmt::{Display, Formatter},
    num::ParseIntError,
    str::FromStr,
};
use toml::Value;

use crate::error::Error;

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct AnniDate(
    u16, /* year */
    Option<(u8 /* month */, Option<u8 /* day */>)>,
);

impl AnniDate {
    pub const UNKNOWN: AnniDate = AnniDate(0, None);

    pub fn year(&self) -> u16 {
        self.0
    }

    pub fn month(&self) -> Option<u8> {
        self.1.map(|(month, _)| month)
    }

    pub fn day(&self) -> Option<u8> {
        self.1.and_then(|(_, day)| day)
    }
}

impl Serialize for AnniDate {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if let (Some(month), Some(day)) = (self.month(), self.day()) {
            let date = toml_edit::Datetime {
                date: Some(toml_edit::Date {
                    year: self.year(),
                    month,
                    day,
                }),
                time: None,
                offset: None,
            };
            toml_edit::Datetime::serialize(&date, serializer)
        } else {
            let mut result = format!("{:04}", self.year());
            if let Some(month) = self.month() {
                result += &format!("-{:02}", month);
            }
            Value::serialize(&Value::String(result), serializer)
        }
    }
}

impl<'de> Deserialize<'de> for AnniDate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de;

        let value = Value::deserialize(deserializer)?;
        let result = match value {
            Value::Datetime(datetime) => {
                let date = datetime.to_string();
                let split = date.split('-').collect::<Vec<_>>();
                Self::from_parts(split[0], split[1], split[2])
                    .map_err(|_| de::Error::custom("Invalid date format"))?
            }
            Value::String(date) => {
                Self::from_str(&date).map_err(|_| de::Error::custom("Invalid date format"))?
            }
            _ => {
                return Err(de::Error::custom("Invalid date format"));
            }
        };
        Ok(result)
    }
}

impl FromStr for AnniDate {
    type Err = Error;

    fn from_str(date: &str) -> Result<Self, Self::Err> {
        // yyyy-mm-dd
        let parts = date.split('-').collect::<Vec<_>>();
        if parts.len() == 1 {
            Self::from_parts(parts[0], "0", "0")
        } else if parts.len() == 2 {
            Self::from_parts(parts[0], parts[1], "0")
        } else {
            Self::from_parts(parts[0], parts[1], parts[2])
        }
        .map_err(|_| Error::InvalidDate(date.to_string()))
    }
}

impl AnniDate {
    pub fn new(year: u16, month: u8, day: u8) -> Self {
        if year == 0 {
            Self::UNKNOWN
        } else {
            Self(year, Some((month, Some(day))))
        }
    }

    pub fn from_parts(y: &str, m: &str, d: &str) -> Result<Self, ParseIntError> {
        let year_offset = if y.len() == 2 {
            // In August 1982, the first compact disc was manufactured.
            // It was then released in October 1982 and branded as Digital Audio Compact Disc.
            // So [82, ) implies 19xx, others imply 20xx
            if u8::from_str(y).unwrap() >= 82 {
                1900
            } else {
                2000
            }
        } else {
            0
        };
        Ok(Self::new(
            year_offset + u16::from_str(y)?,
            u8::from_str(m)?,
            u8::from_str(d)?,
        ))
    }

    /// Print date in short format, e.g. 190130
    pub fn to_short_string(&self) -> String {
        format!(
            "{:02}{:02}{:02}",
            self.year() % 100,
            self.month().unwrap_or(0),
            self.day().unwrap_or(0)
        )
    }
}

impl Display for AnniDate {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match (self.month(), self.day()) {
            (Some(month), Some(day)) => write!(f, "{}-{:02}-{:02}", self.year(), month, day),
            (Some(month), None) => write!(f, "{}-{:02}", self.year(), month),
            _ => write!(f, "{}", self.year()),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_date() {
        use super::*;

        let date = AnniDate::from_parts("19", "01", "30").unwrap();
        assert_eq!(date.year(), 2019);
        assert_eq!(date.month(), Some(1));
        assert_eq!(date.day(), Some(30));
        assert_eq!(date.to_string(), "2019-01-30");
        assert_eq!(date.to_short_string(), "190130");

        // though there's no cd released in 201-AD
        let date = AnniDate::from_parts("201", "1", "31").unwrap();
        assert_eq!(date.year(), 201);
        assert_eq!(date.month(), Some(1));
        assert_eq!(date.day(), Some(31));
        assert_eq!(date.to_string(), "201-01-31");
        assert_eq!(date.to_short_string(), "010131");

        let date = AnniDate::from_parts("1919", "08", "10").unwrap();
        assert_eq!(date.year(), 1919);
        assert_eq!(date.month(), Some(8));
        assert_eq!(date.day(), Some(10));
        assert_eq!(date.to_string(), "1919-08-10");
        assert_eq!(date.to_short_string(), "190810");

        let date = AnniDate::from_parts("1982", "08", "10").unwrap();
        assert_eq!(date.year(), 1982);
        assert_eq!(date.month(), Some(8));
        assert_eq!(date.day(), Some(10));
        assert_eq!(date.to_string(), "1982-08-10");
        assert_eq!(date.to_short_string(), "820810");
    }
}
