use serde::{Serialize, Deserialize, Deserializer, Serializer};
use std::fmt::{Display, Formatter};
use toml::Value;

#[derive(PartialEq, Debug)]
pub struct AnniDate {
    year: u32,
    month: u8,
    day: u8,
}

impl Serialize for AnniDate {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer {
        if self.month > 0 && self.day > 0 {
            use std::str::FromStr;
            let date = toml::value::Datetime::from_str(&self.to_string()).unwrap();
            Value::serialize(&Value::Datetime(date), serializer)
        } else {
            let mut table = toml::value::Table::new();
            table.insert("year".to_string(), Value::Integer(self.year as i64));
            if self.month > 0 {
                table.insert("month".to_string(), Value::Integer(self.month as i64));
                if self.day > 0 {
                    table.insert("day".to_string(), Value::Integer(self.day as i64));
                }
            }
            Value::serialize(&Value::Table(table), serializer)
        }
    }
}

impl<'de> Deserialize<'de> for AnniDate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'de> {
        use serde::de;

        let value = Value::deserialize(deserializer)?;
        let result = match value {
            Value::Datetime(datetime) => {
                let date = datetime.to_string();
                let split = date.split('-').collect::<Vec<_>>();
                Self::from_parts(split[0], split[1], split[2])
            }
            Value::Table(table) => {
                let year = table.get("year")
                    .ok_or_else(|| de::Error::custom("Missing field `year`"))?
                    .as_integer()
                    .map(|y| y as u32)
                    .ok_or_else(|| de::Error::custom("Invalid type for field `date.year`"))?;
                let month = table.get("month")
                    .map_or_else(
                        || Ok(0),
                        |m| m.as_integer()
                            .map(|i| i as u8)
                            .ok_or_else(|| de::Error::custom("Invalid type for field `date.month`")),
                    )?;
                let day = table.get("day")
                    .map_or_else(
                        || Ok(0),
                        |m| m.as_integer()
                            .map(|i| i as u8)
                            .ok_or_else(|| de::Error::custom("Invalid type for field `date.day`")),
                    )?;
                if day > 0 && month == 0 {
                    // yy00dd is invalid
                    return Err(de::Error::custom("Invalid date format `yy00dd`!"));
                }

                Self::new(year, month, day)
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
        Self {
            year,
            month,
            day,
        }
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
        Self::new(year_offset + u32::from_str(y).unwrap(), u8::from_str(m).unwrap(), u8::from_str(d).unwrap())
    }
}

impl Display for AnniDate {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}