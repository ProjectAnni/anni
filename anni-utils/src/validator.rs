pub type Validator = fn(&str) -> bool;

pub fn pass_validator(_str: &str) -> bool {
    true
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

#[cfg(test)]
mod tests {
    use crate::validator::{trim_validator, date_validator};

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
}
