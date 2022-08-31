use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::{Display, Formatter};
use toml_edit::easy::Value;

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct AnniDate {
    year: u32,
    month: u8,
    day: u8,
}

impl Serialize for AnniDate {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if self.month > 0 && self.day > 0 {
            use std::str::FromStr;
            let date = toml_edit::easy::value::Datetime::from_str(&self.to_string()).unwrap();
            Value::serialize(&Value::Datetime(date), serializer)
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
    pub fn new(year: u32, month: u8, day: u8) -> Self {
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
            year_offset + u32::from_str(y).unwrap(),
            u8::from_str(m).unwrap_or(0),
            u8::from_str(d).unwrap_or(0),
        )
    }
}

impl Display for AnniDate {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.day == 0 {
            if self.month == 0 {
                write!(f, "{:04}", self.year)
            } else {
                write!(f, "{:04}-{:02}", self.year, self.month)
            }
        } else {
            write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_date() {
        use super::*;

        let anni_date = AnniDate::from_parts("19", "01", "30");
        assert_eq!(anni_date.year, 2019);
        assert_eq!(anni_date.month, 1);
        assert_eq!(anni_date.day, 30);

        let anni_date = AnniDate::from_parts("2019", "01", "30");
        assert_eq!(anni_date.year, 2019);
        assert_eq!(anni_date.month, 1);
        assert_eq!(anni_date.day, 30);
    }
}
