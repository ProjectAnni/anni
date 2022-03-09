use anni_artist::ArtistList;
use regex::Regex;
use serde::de::Error;
use serde::{Deserialize, Deserializer};
use std::str::FromStr;

pub struct Validator(&'static str, fn(&str) -> ValidateResult);

impl Validator {
    #[inline]
    pub fn name(&self) -> &'static str {
        self.0
    }

    #[inline]
    pub fn validate(&self, input: &str) -> ValidateResult {
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
            _ => Err(()),
        }
    }
}

impl<'de> Deserialize<'de> for Validator {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
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

pub struct ValidateResult {
    pub valid: bool,
    pub warning: bool,
    pub message: Option<String>,
}

impl ValidateResult {
    fn new(valid: bool, is_warning: bool) -> Self {
        Self {
            valid,
            warning: is_warning,
            message: None,
        }
    }
}

pub fn number_validator(str: &str) -> ValidateResult {
    let pass = str.chars().all(|c| c.is_numeric());
    ValidateResult::new(pass, false)
}

pub fn trim_validator(str: &str) -> ValidateResult {
    let mut is_start = true;
    let mut is_empty = false;
    for c in str.chars() {
        is_empty = c.is_whitespace();
        if is_start && is_empty {
            break;
        }
        is_start = false;
    }
    let pass = !is_empty;
    ValidateResult::new(pass, false)
}

pub fn date_validator(str: &str) -> ValidateResult {
    // 2021-01-01
    // 0123456789
    let mut mode = 0;
    for c in str.chars() {
        if mode > 9 || (!c.is_numeric() && c != '-') {
            return ValidateResult::new(false, false);
        }
        if c == '-' {
            if mode != 4 && mode != 7 {
                return ValidateResult::new(false, false);
            }
        } else if !c.is_numeric() {
            return ValidateResult::new(false, false);
        }
        mode += 1;
    }
    let is_year_month_day = mode == 10;
    let is_year_month = mode == 7;
    let is_year = mode == 4;
    if is_year_month_day {
        ValidateResult::new(true, false)
    } else if is_year_month {
        ValidateResult {
            valid: true,
            warning: true,
            message: Some("Empty day field, could it be more accurate?".to_string()),
        }
    } else if is_year {
        ValidateResult {
            valid: true,
            warning: true,
            message: Some("Empty month and day fields, could it be more accurate?".to_string()),
        }
    } else {
        ValidateResult::new(false, false)
    }
}

pub fn artist_validator(str: &str) -> ValidateResult {
    match ArtistList::parse(str) {
        Ok(_) => ValidateResult::new(true, false),
        Err(err) => {
            log::debug!("ArtistList parse error: {}", err);
            ValidateResult::new(false, false)
        }
    }
}

lazy_static::lazy_static! {
    static ref DOTS: Regex = Regex::new(r"[\u{0087}\u{0387}\u{16eb}\u{2022}\u{2027}\u{2218}\u{2219}\u{22c5}\u{25e6}\u{2981}\u{2e30}\u{2e31}\u{ff65}\u{10101}]").unwrap();
}

/// http://www.0x08.org/posts/middle-dot
pub fn middle_dot_validator(input: &str) -> ValidateResult {
    let pass = !DOTS.is_match(input);
    ValidateResult::new(pass, false)
}

pub fn middle_dot_replace(input: &str) -> String {
    DOTS.replace_all(input, "\u{30fb}").to_string()
}

#[cfg(test)]
mod tests {
    use crate::validator::{
        date_validator, middle_dot_replace, middle_dot_validator, trim_validator,
    };

    #[test]
    fn trim_exist() {
        assert!(!trim_validator("  1234").valid);
        assert!(!trim_validator("1234   ").valid);
        assert!(!trim_validator("\n1234").valid);
    }

    #[test]
    fn trim_not_exist() {
        assert!(trim_validator("1234").valid);
    }

    #[test]
    fn date_valid() {
        assert!(date_validator("2021-01-01").valid);
    }

    #[test]
    fn date_invalid() {
        assert!(!date_validator("2020-01-012").valid);
        assert!(!date_validator("2020~01-01").valid);
        assert!(!date_validator("?").valid);
    }

    #[test]
    fn middle_dot_detect() {
        assert!(middle_dot_validator("123").valid);

        assert!(!middle_dot_validator("\u{0087}").valid);
        assert!(!middle_dot_validator("\u{0087}").valid);
        assert!(!middle_dot_validator("\u{0387}").valid);
        assert!(!middle_dot_validator("\u{16eb}").valid);
        assert!(!middle_dot_validator("\u{2022}").valid);
        assert!(!middle_dot_validator("\u{2027}").valid);
        assert!(!middle_dot_validator("\u{2218}").valid);
        assert!(!middle_dot_validator("\u{2219}").valid);
        assert!(!middle_dot_validator("\u{22c5}").valid);
        assert!(!middle_dot_validator("\u{25e6}").valid);
        assert!(!middle_dot_validator("\u{2981}").valid);
        assert!(!middle_dot_validator("\u{2e30}").valid);
        assert!(!middle_dot_validator("\u{2e31}").valid);
        assert!(!middle_dot_validator("\u{ff65}").valid);
        assert!(!middle_dot_validator("\u{10101}").valid);
    }

    #[test]
    fn middle_dot_replace_all() {
        assert_eq!(
            middle_dot_replace("1\u{0087}2\u{0387}3\u{16eb}4\u{2022}5\u{2027}6\u{2218}7\u{2219}8\u{22c5}9\u{25e6}1\u{2981}2\u{2e30}3\u{2e31}4\u{ff65}5\u{10101}6"),
            "1・2・3・4・5・6・7・8・9・1・2・3・4・5・6"
        );
    }
}
