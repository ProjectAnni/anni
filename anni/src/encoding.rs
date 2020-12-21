macro_rules! check_middle_dot {
    ($input: ident, $dot: expr) => {
        if $input.contains($dot) { return Some($dot.escape_unicode().to_string()) }
    };
}

/// http://www.0x08.org/posts/middle-dot
pub(crate) fn middle_dot_valid(input: &str) -> Option<String> {
    check_middle_dot!(input, "\u{0087}");
    check_middle_dot!(input, "\u{0387}");
    check_middle_dot!(input, "\u{16eb}");
    check_middle_dot!(input, "\u{2022}");
    check_middle_dot!(input, "\u{2027}");
    check_middle_dot!(input, "\u{2218}");
    check_middle_dot!(input, "\u{2219}");
    check_middle_dot!(input, "\u{22c5}");
    check_middle_dot!(input, "\u{25e6}");
    check_middle_dot!(input, "\u{2981}");
    check_middle_dot!(input, "\u{2e30}");
    check_middle_dot!(input, "\u{2e31}");
    check_middle_dot!(input, "\u{ff65}");
    check_middle_dot!(input, "\u{10101}");
    None
}