use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::{Display, Formatter};
use toml_edit::easy::Value;

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct AnniDate {
    year: u16,
    month: u8,
    day: u8,
}

impl Serialize for AnniDate {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if self.month > 0 && self.day > 0 {
            let date = toml_edit::Datetime {
                date: Some(toml_edit::Date {
                    year: self.year,
                    month: self.month,
                    day: self.day,
                }),
                time: None,
                offset: None,
            };
            toml_edit::Datetime::serialize(&date, serializer)
        } else {
            let mut result = format!("{:04}", self.year);
            if self.month > 0 {
                result += &format!("-{:02}", self.month);
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
            }
            Value::String(date) => {
                // yyyy-mm-dd
                let parts = date.split('-').collect::<Vec<_>>();
                if parts.len() == 1 {
                    Self::from_parts(parts[0], "0", "0")
                } else if parts.len() == 2 {
                    Self::from_parts(parts[0], parts[1], "0")
                } else {
                    Self::from_parts(parts[0], parts[1], parts[2])
                }
            }
            _ => {
                return Err(de::Error::custom("Invalid date format"));
            }
        };
        Ok(result)
    }
}

impl AnniDate {
    pub fn new(year: u16, month: u8, day: u8) -> Self {
        Self { year, month, day }
    }

    pub fn from_parts(y: &str, m: &str, d: &str) -> Self {
        use std::str::FromStr;

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
        Self::new(
            year_offset + u16::from_str(y).unwrap(),
            u8::from_str(m).unwrap_or(0),
            u8::from_str(d).unwrap_or(0),
        )
    }
}

impl Display for AnniDate {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.day == 0 {
            if self.month == 0 {
                write!(f, "{}", self.year)
            } else {
                write!(f, "{}-{:02}", self.year, self.month)
            }
        } else {
            write!(f, "{}-{:02}-{:02}", self.year, self.month, self.day)
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_date() {
        use super::*;

        let date = AnniDate::from_parts("19", "01", "30");
        assert_eq!(date.year, 2019);
        assert_eq!(date.month, 1);
        assert_eq!(date.day, 30);
        assert_eq!(date.to_string(), "2019-01-30");

        // though there's no cd released in 201-AD
        let date = AnniDate::from_parts("201", "1", "31");
        assert_eq!(date.year, 201);
        assert_eq!(date.month, 1);
        assert_eq!(date.day, 31);
        assert_eq!(date.to_string(), "201-01-31");

        let date = AnniDate::from_parts("1919", "08", "10");
        assert_eq!(date.year, 1919);
        assert_eq!(date.month, 8);
        assert_eq!(date.day, 10);

        let date = AnniDate::from_parts("1982", "08", "10");
        assert_eq!(date.year, 1982);
        assert_eq!(date.month, 8);
        assert_eq!(date.day, 10);
    }
}
