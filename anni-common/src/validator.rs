use crate::artist::ArtistList;
use std::str::FromStr;
use regex::Regex;
use serde::{Deserialize, Deserializer};
use serde::de::Error;

pub struct Validator(&'static str, fn(&str) -> bool);

impl Validator {
    #[inline]
    pub fn name(&self) -> &'static str {
        self.0
    }

    #[inline]
    pub fn validate(&self, input: &str) -> bool {
        self.1(input)
    }
}

impl FromStr for Validator {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "number" => Ok(Self("number", number_validator)),
            "trim" => Ok(Self("trim", trim_validator)),
            "date" => Ok(Self("date", date_validator)),
            "artist" => Ok(Self("artist", artist_validator)),
            "dot" => Ok(Self("dot", middle_dot_validator)),
            _ => Err(())
        }
    }
}

impl<'de> Deserialize<'de> for Validator {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'de>
    {
        let s = String::deserialize(deserializer)?;
        Validator::from_str(s.as_str()).map_err(|_| D::Error::custom(s))
    }
}

impl std::fmt::Debug for Validator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

pub fn number_validator(str: &str) -> bool {
    str.chars().all(|c| c.is_numeric())
}

pub fn trim_validator(str: &str) -> bool {
    let mut is_start = true;
    let mut is_empty = false;
    for c in str.chars() {
        is_empty = c.is_whitespace();
        if is_start && is_empty {
            break;
        }
        is_start = false;
    }
    !is_empty
}

pub fn date_validator(str: &str) -> bool {
    // 2021-01-01
    // 0123456789
    let mut mode = 0;
    for c in str.chars() {
        if mode > 9 || (!c.is_numeric() && c != '-') {
            return false;
        }
        if c == '-' {
            if mode != 4 && mode != 7 {
                return false;
            }
        } else if !c.is_numeric() {
            return false;
        }
        mode += 1;
    }
    return mode == 10;
}

pub fn artist_validator(str: &str) -> bool {
    ArtistList::is_valid(str)
}

lazy_static::lazy_static! {
    static ref DOTS: Regex = Regex::new(r"[\u{0087}\u{0387}\u{16eb}\u{2022}\u{2027}\u{2218}\u{2219}\u{22c5}\u{25e6}\u{2981}\u{2e30}\u{2e31}\u{ff65}\u{10101}]").unwrap();
}

/// http://www.0x08.org/posts/middle-dot
pub fn middle_dot_validator(input: &str) -> bool {
    !DOTS.is_match(input)
}

pub fn middle_dot_replace(input: &str) -> String {
    DOTS.replace_all(input, "\u{30fb}").to_string()
}

#[cfg(test)]
mod tests {
    use anni_common::validator::{trim_validator, date_validator, middle_dot_validator, middle_dot_replace};

    #[test]
    fn trim_exist() {
        assert_eq!(false, trim_validator("  1234"));
        assert_eq!(false, trim_validator("1234   "));
        assert_eq!(false, trim_validator("\n1234"));
    }

    #[test]
    fn trim_not_exist() {
        assert_eq!(true, trim_validator("1234"));
    }

    #[test]
    fn date_valid() {
        assert_eq!(true, date_validator("2021-01-01"));
    }

    #[test]
    fn date_invalid() {
        assert_eq!(false, date_validator("2020-01-012"));
        assert_eq!(false, date_validator("2020~01-01"));
        assert_eq!(false, date_validator("2020"));
        assert_eq!(false, date_validator("?"));
    }

    #[test]
    fn middle_dot_detect() {
        assert_eq!(true, middle_dot_validator("123"));

        assert_eq!(false, middle_dot_validator("\u{0087}"));
        assert_eq!(false, middle_dot_validator("\u{0087}"));
        assert_eq!(false, middle_dot_validator("\u{0387}"));
        assert_eq!(false, middle_dot_validator("\u{16eb}"));
        assert_eq!(false, middle_dot_validator("\u{2022}"));
        assert_eq!(false, middle_dot_validator("\u{2027}"));
        assert_eq!(false, middle_dot_validator("\u{2218}"));
        assert_eq!(false, middle_dot_validator("\u{2219}"));
        assert_eq!(false, middle_dot_validator("\u{22c5}"));
        assert_eq!(false, middle_dot_validator("\u{25e6}"));
        assert_eq!(false, middle_dot_validator("\u{2981}"));
        assert_eq!(false, middle_dot_validator("\u{2e30}"));
        assert_eq!(false, middle_dot_validator("\u{2e31}"));
        assert_eq!(false, middle_dot_validator("\u{ff65}"));
        assert_eq!(false, middle_dot_validator("\u{10101}"));
    }

    #[test]
    fn middle_dot_replace_all() {
        assert_eq!(
            middle_dot_replace("1\u{0087}2\u{0387}3\u{16eb}4\u{2022}5\u{2027}6\u{2218}7\u{2219}8\u{22c5}9\u{25e6}1\u{2981}2\u{2e30}3\u{2e31}4\u{ff65}5\u{10101}6"),
            "1・2・3・4・5・6・7・8・9・1・2・3・4・5・6"
        );
    }
}
