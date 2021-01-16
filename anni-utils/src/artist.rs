pub struct Artist<'a> {
    pub name: &'a str,
    pub alias: Option<Vec<Artist<'a>>>,
}

pub struct ArtistList<'a> {
    pub artists: Vec<Artist<'a>>,
}

enum Symbol {
    LBracket,
    Normal,
    Separator,
    RBracket,
}

impl Symbol {
    pub fn is_symbol(c: char) -> bool {
        c == '（' || c == '）' || c == '、'
    }
}

impl From<char> for Symbol {
    fn from(c: char) -> Self {
        if c == '（' {
            Symbol::LBracket
        } else if c == '）' {
            Symbol::RBracket
        } else if c == '、' {
            Symbol::Separator
        } else {
            Symbol::Normal
        }
    }
}

impl Symbol {
    #[inline]
    pub fn is_left_bracket_or_normal(c: char) -> bool {
        c != '）' && c != '、'
    }
}

impl<'a> ArtistList<'a> {
    /// Artist List has the following format
    /// `ArtistA（MemberB、MemberC、SubArtistD（MemberE、MemberF））、ArtistG`
    /// So:
    /// 1. When we meet `（`, a subparse should start until `）` does not meet
    /// 2. We can almost ignore `、` when validating if a input is valid
    /// 3. Structures like `（（` are invalid
    /// 4. Structures like `、（`, `（、` and `、）` are invalid
    /// 5. That's to say, when we meet two consecutive symbols, only `））` and `）、` are valid
    fn is_str_valid<T: AsRef<str>>(input: T, is_start: bool) -> (bool, usize) {
        let input = input.as_ref();
        if input.is_empty() {
            return (false, 0);
        }

        let mut last_symbol = Symbol::LBracket;
        let mut skip_num = 0usize;
        let mut i = 0usize;
        let mut last_rbracket = false;
        for c in input.chars().into_iter() {
            // Count char size
            i += c.len_utf8();

            // Skip mode
            if skip_num > 0 {
                skip_num -= c.len_utf8();
                continue;
            }

            if last_rbracket {
                if Symbol::is_left_bracket_or_normal(c) {
                    return (false, i);
                } else {
                    last_rbracket = false;
                }
            }

            // Match mode
            match last_symbol {
                Symbol::Normal => {
                    match Symbol::from(c) {
                        Symbol::Normal => continue,
                        Symbol::LBracket => {
                            let (valid, skip) = ArtistList::is_str_valid(&input[i..], false);
                            if !valid {
                                return (false, i + skip);
                            }
                            skip_num = skip;
                            last_symbol = Symbol::Normal;
                            last_rbracket = true;
                        }
                        Symbol::RBracket => return (!is_start, i),
                        Symbol::Separator => last_symbol = Symbol::Separator,
                    }
                }
                Symbol::LBracket | Symbol::Separator => {
                    // `、` with any symbol is invalid
                    // `、（`, `、）`, `、、`
                    if Symbol::is_symbol(c) {
                        return (false, i);
                    }
                    last_symbol = Symbol::Normal;
                }
                Symbol::RBracket => {
                    // RBracket can't accept LBracket
                    if Symbol::is_left_bracket_or_normal(c) {
                        return (false, i);
                    }
                    last_symbol = Symbol::from(c);
                }
            }
        }
        // Meet the end of input
        match last_symbol {
            // Normal char and RBrackets are valid only at top level
            Symbol::Normal | Symbol::RBracket => (is_start, i),
            // LBracket / Separator are invalid
            _ => (false, i),
        }
    }

    pub fn is_valid(input: &str) -> bool {
        let (result, offset) = ArtistList::is_str_valid(input, true);
        result && offset == input.len()
    }
}

#[cfg(test)]
mod tests {
    use crate::artist::ArtistList;

    #[test]
    fn valid_artist_list() {
        assert_eq!(true, ArtistList::is_valid("水瀬いのり"));
        assert_eq!(true, ArtistList::is_valid("ArtistA（MemberB、MemberC）"));
        assert_eq!(true, ArtistList::is_valid("ArtistA（MemberB、MemberC、SubArtistD（MemberE、MemberF））"));
        assert_eq!(true, ArtistList::is_valid("ArtistA（MemberB、MemberC、SubArtistD（MemberE、MemberF））、ArtistG"));
    }

    #[test]
    fn invalid_artist_list() {
        assert_eq!(false, ArtistList::is_valid("水瀬いのり、"));
        assert_eq!(false, ArtistList::is_valid("、水瀬いのり"));
        assert_eq!(false, ArtistList::is_valid("水瀬いのり（"));
        assert_eq!(false, ArtistList::is_valid("水瀬いのり）"));
        assert_eq!(false, ArtistList::is_valid("水瀬いのり））"));
        assert_eq!(false, ArtistList::is_valid("（水瀬いのり"));
        assert_eq!(false, ArtistList::is_valid("ArtistA（MemberB、MemberC、）"));
        assert_eq!(false, ArtistList::is_valid("ArtistA（MemberB、MemberC）ArtistB"));
        assert_eq!(false, ArtistList::is_valid("ArtistA（MemberB、MemberC"));
        assert_eq!(false, ArtistList::is_valid("ArtistA（MemberB、MemberC））"));
        assert_eq!(false, ArtistList::is_valid("ArtistA（SubArtistD（MemberE、MemberF）"));
        assert_eq!(false, ArtistList::is_valid("ArtistA（SubArtistD（MemberE、MemberF）））"));
    }
}

