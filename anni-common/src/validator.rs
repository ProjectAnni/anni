use anni_artist::ArtistList;
use once_cell::sync::Lazy;
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
            "tidle" => Ok(Self("tidle", tidal_validator)),
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

#[derive(Default, Debug, Deserialize)]
#[serde(transparent)]
pub struct ValidatorList(Vec<Validator>);

impl ValidatorList {
    pub fn new(validators: &[&str]) -> Option<Self> {
        validators
            .iter()
            .map(|v| Validator::from_str(v))
            .collect::<Result<_, _>>()
            .map(ValidatorList)
            .ok()
    }

    pub fn validate(&self, input: &str) -> Vec<(&'static str, ValidateResult)> {
        self.0
            .iter()
            .map(|v| (v.0, v.1(input)))
            .filter(|v| !v.1.is_pass())
            .collect()
    }
}

pub enum ValidateResult {
    Pass,
    Warning(String),
    Error(String),
}

impl ValidateResult {
    fn pass() -> Self {
        Self::Pass
    }

    fn pass_or(pass: bool, message: String) -> Self {
        if pass {
            Self::Pass
        } else {
            Self::Error(message)
        }
    }

    fn warn(message: String) -> Self {
        Self::Warning(message)
    }

    fn fail(message: String) -> Self {
        Self::Error(message)
    }

    pub fn is_pass(&self) -> bool {
        matches!(self, Self::Pass)
    }

    pub fn into_message(self) -> String {
        match self {
            Self::Warning(m) => m,
            Self::Error(m) => m,
            Self::Pass => unreachable!(),
        }
    }
}

pub fn number_validator(str: &str) -> ValidateResult {
    let pass = str.chars().all(|c| c.is_numeric());
    ValidateResult::pass_or(pass, "not a number".to_string())
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
    ValidateResult::pass_or(pass, "whitespaces need to be trimmed".to_string())
}

pub fn date_validator(str: &str) -> ValidateResult {
    // 2021-01-01
    // 0123456789
    let mut mode = 0;
    for c in str.chars() {
        if mode > 9 || (!c.is_numeric() && c != '-') {
            return ValidateResult::fail("invalid date".to_string());
        }
        if c == '-' {
            if mode != 4 && mode != 7 {
                return ValidateResult::fail("invalid date".to_string());
            }
        } else if !c.is_numeric() {
            return ValidateResult::fail("invalid date".to_string());
        }
        mode += 1;
    }
    let is_year_month_day = mode == 10;
    let is_year_month = mode == 7;
    let is_year = mode == 4;
    if is_year_month_day {
        ValidateResult::pass()
    } else if is_year_month {
        ValidateResult::warn("Empty day field, could it be more accurate?".to_string())
    } else if is_year {
        ValidateResult::warn("Empty month and day fields, could it be more accurate?".to_string())
    } else {
        ValidateResult::fail("invalid date".to_string())
    }
}

pub fn artist_validator(str: &str) -> ValidateResult {
    match ArtistList::parse(str) {
        Ok(_) => ValidateResult::pass(),
        Err(err) => {
            log::debug!("ArtistList parse error: {}", err);
            ValidateResult::fail(err)
        }
    }
}

static DOTS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"[\u{00B7}\u{0387}\u{16eb}\u{2022}\u{2027}\u{2218}\u{2219}\u{22c5}\u{25e6}\u{2981}\u{2e30}\u{2e31}\u{ff65}\u{10101}]").unwrap()
});

/// http://www.0x08.org/posts/middle-dot
pub fn middle_dot_validator(input: &str) -> ValidateResult {
    let pass = !DOTS.is_match(input);
    ValidateResult::pass_or(pass, "invalid dots detected".to_string())
}

pub fn middle_dot_replace(input: &str) -> String {
    DOTS.replace_all(input, "\u{30fb}").to_string()
}

pub fn tidal_validator(input: &str) -> ValidateResult {
    let pass = !input.contains('\u{301c}');
    ValidateResult::pass_or(pass, "invalid tidal detected".to_string())
}

pub fn tidal_replace(input: &str) -> String {
    input.replace('\u{301c}', "\u{ff5e}")
}

#[cfg(test)]
mod tests {
    use crate::validator::{
        date_validator, middle_dot_replace, middle_dot_validator, trim_validator, ValidateResult,
    };

    #[test]
    fn trim_exist() {
        assert!(matches!(trim_validator("  1234"), ValidateResult::Error(_)));
        assert!(matches!(
            trim_validator("1234   "),
            ValidateResult::Error(_)
        ));
        assert!(matches!(trim_validator("\n1234"), ValidateResult::Error(_)));
    }

    #[test]
    fn trim_not_exist() {
        assert!(matches!(trim_validator("1234"), ValidateResult::Pass));
    }

    #[test]
    fn test_date_validator() {
        assert!(matches!(date_validator("2021-01-01"), ValidateResult::Pass));

        assert!(matches!(
            date_validator("2020-01-012"),
            ValidateResult::Error(_)
        ));
        assert!(matches!(
            date_validator("2020~01-01"),
            ValidateResult::Error(_)
        ));
        assert!(matches!(date_validator("?"), ValidateResult::Error(_)));
    }

    #[test]
    fn middle_dot_detect() {
        assert!(matches!(middle_dot_validator("123"), ValidateResult::Pass));

        assert!(matches!(
            middle_dot_validator("\u{00B7}"),
            ValidateResult::Error(_)
        ));
        assert!(matches!(
            middle_dot_validator("\u{0387}"),
            ValidateResult::Error(_)
        ));
        assert!(matches!(
            middle_dot_validator("\u{16eb}"),
            ValidateResult::Error(_)
        ));
        assert!(matches!(
            middle_dot_validator("\u{2022}"),
            ValidateResult::Error(_)
        ));
        assert!(matches!(
            middle_dot_validator("\u{2027}"),
            ValidateResult::Error(_)
        ));
        assert!(matches!(
            middle_dot_validator("\u{2218}"),
            ValidateResult::Error(_)
        ));
        assert!(matches!(
            middle_dot_validator("\u{2219}"),
            ValidateResult::Error(_)
        ));
        assert!(matches!(
            middle_dot_validator("\u{22c5}"),
            ValidateResult::Error(_)
        ));
        assert!(matches!(
            middle_dot_validator("\u{25e6}"),
            ValidateResult::Error(_)
        ));
        assert!(matches!(
            middle_dot_validator("\u{2981}"),
            ValidateResult::Error(_)
        ));
        assert!(matches!(
            middle_dot_validator("\u{2e30}"),
            ValidateResult::Error(_)
        ));
        assert!(matches!(
            middle_dot_validator("\u{2e31}"),
            ValidateResult::Error(_)
        ));
        assert!(matches!(
            middle_dot_validator("\u{ff65}"),
            ValidateResult::Error(_)
        ));
        assert!(matches!(
            middle_dot_validator("\u{10101}"),
            ValidateResult::Error(_)
        ));
    }

    #[test]
    fn middle_dot_replace_all() {
        assert_eq!(
            middle_dot_replace("1\u{00B7}2\u{0387}3\u{16eb}4\u{2022}5\u{2027}6\u{2218}7\u{2219}8\u{22c5}9\u{25e6}1\u{2981}2\u{2e30}3\u{2e31}4\u{ff65}5\u{10101}6"),
            "1・2・3・4・5・6・7・8・9・1・2・3・4・5・6"
        );
    }
}
