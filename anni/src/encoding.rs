use regex::{Regex};

lazy_static::lazy_static! {
    static ref DOTS: Regex = Regex::new(r"[\u{0087}\u{0387}\u{16eb}\u{2022}\u{2027}\u{2218}\u{2219}\u{22c5}\u{25e6}\u{2981}\u{2e30}\u{2e31}\u{ff65}\u{10101}]").unwrap();
}

/// http://www.0x08.org/posts/middle-dot
pub(crate) fn middle_dot_valid(input: &str) -> bool {
    !DOTS.is_match(input)
}

pub(crate) fn middle_dot_replace(input: &str) -> String {
    DOTS.replace_all(input, "\u{30fb}").to_string()
}

#[cfg(test)]
mod test {
    use crate::encoding::{middle_dot_valid, middle_dot_replace};

    #[test]
    fn middle_dot_detect() {
        assert_eq!(true, middle_dot_valid("123"));

        assert_eq!(false, middle_dot_valid("\u{0087}"));
        assert_eq!(false, middle_dot_valid("\u{0087}"));
        assert_eq!(false, middle_dot_valid("\u{0387}"));
        assert_eq!(false, middle_dot_valid("\u{16eb}"));
        assert_eq!(false, middle_dot_valid("\u{2022}"));
        assert_eq!(false, middle_dot_valid("\u{2027}"));
        assert_eq!(false, middle_dot_valid("\u{2218}"));
        assert_eq!(false, middle_dot_valid("\u{2219}"));
        assert_eq!(false, middle_dot_valid("\u{22c5}"));
        assert_eq!(false, middle_dot_valid("\u{25e6}"));
        assert_eq!(false, middle_dot_valid("\u{2981}"));
        assert_eq!(false, middle_dot_valid("\u{2e30}"));
        assert_eq!(false, middle_dot_valid("\u{2e31}"));
        assert_eq!(false, middle_dot_valid("\u{ff65}"));
        assert_eq!(false, middle_dot_valid("\u{10101}"));
    }

    #[test]
    fn middle_dot_replace_all() {
        assert_eq!(
            middle_dot_replace("1\u{0087}2\u{0387}3\u{16eb}4\u{2022}5\u{2027}6\u{2218}7\u{2219}8\u{22c5}9\u{25e6}1\u{2981}2\u{2e30}3\u{2e31}4\u{ff65}5\u{10101}6"),
            "1・2・3・4・5・6・7・8・9・1・2・3・4・5・6"
        );
    }
}