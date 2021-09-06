//! Anni describes artist in a nested way
//! For example:
//! ```text
//! Petit Rabbit's（ココア（佐倉綾音）、チノ（水瀬いのり）、リゼ（種田梨沙）、千夜（佐藤聡美）、シャロ（内田真礼））
//! ```
//! `Petit Rabbit's` is an artist name, and everything in brackets are artist alias
//! `Petit Rabbit's` contains five members, so alias has the meaning of 'containing'
//! If there's only one member, then alias is exactly artist **alias**
//!
//! Then we know, a valid artist field is a list of valid artists,
//! and a valid artist contains an artist name and a list of alias, which is a list of valid artists
//! So we abstract the artist model to Artist and ArtistList. Each artist has an optional alias field.

use std::fmt::{Display, Formatter, Debug};
use thiserror::Error;

pub struct AnniArtist {
    inner: String,
}

impl AnniArtist {
    pub fn new(artist: String) -> Result<Self, ArtistParseError> {
        if !ArtistList::is_valid(&artist) {
            return Err(ArtistParseError::InvalidArtistFormat);
        }
        Ok(Self {
            inner: artist,
        })
    }

    pub fn to_artist_list(&self) -> ArtistList {
        let (list, _) = ArtistList::from_artist_str(&self.inner);
        list
    }
}

impl Display for AnniArtist {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.inner, f)
    }
}

#[derive(Error, Debug)]
pub enum ArtistParseError {
    #[error("invalid artist format")]
    InvalidArtistFormat,
}

pub struct Artist<'a> {
    pub name: &'a str,
    pub alias: Option<ArtistList<'a>>,
}

// TODO: replace with TryFrom/TryInto
impl<'a> Artist<'a> {
    pub fn from_artist_str(input: &'a str) -> (Self, &str) {
        for (offset, ch) in input.char_indices() {
            match Symbol::from(ch) {
                Symbol::Normal => {}
                Symbol::LBracket => {
                    // look for ArtistList
                    let (alias, remaining) = ArtistList::from_artist_str(&input[(offset + '（'.len_utf8())..]);
                    let (name, _) = input.split_at(offset);
                    return (Self { name, alias: Some(alias) }, remaining);
                }
                Symbol::RBracket => {
                    return (
                        Self {
                            name: &input[..offset],
                            alias: None,
                        },
                        &input[offset..],
                    );
                }
                Symbol::Separator => {
                    panic!("unexpected Separator in Artist");
                }
            }
        }
        panic!("TODO")
    }
}

impl Display for Artist<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name)?;
        if let Some(alias) = &self.alias {
            write!(f, "（{}）", alias)?;
        }
        Ok(())
    }
}

pub struct ArtistList<'a> {
    pub artists: Vec<Artist<'a>>,
}

impl<'a> ArtistList<'a> {
    /// Parse input to ArtistList
    /// Return the list and the remaining &str
    pub fn from_artist_str(mut input: &'a str) -> (Self, &str) {
        let mut artists = Vec::new();
        let mut chars = input.char_indices();
        while let Some((offset, ch)) = chars.next() {
            match Symbol::from(ch) {
                Symbol::Normal => {
                    let (artist, remaining) = Artist::from_artist_str(&input[offset..]);
                    artists.push(artist);
                    input = remaining;
                    chars = remaining.char_indices();
                }
                Symbol::Separator => {}
                Symbol::LBracket => {
                    panic!("unexpected LBracket");
                }
                Symbol::RBracket => {
                    // does not return rbracket
                    return (Self { artists }, &input[(offset + '）'.len_utf8())..]);
                }
            }
        }

        (ArtistList { artists }, input)
    }
}

impl Display for ArtistList<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        assert!(!self.artists.is_empty());
        let mut iter = self.artists.iter();
        iter.next().unwrap().fmt(f)?;
        for a in iter {
            f.write_str("、")?;
            a.fmt(f)?;
        }
        Ok(())
    }
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
    use crate::artist::{ArtistList, AnniArtist};

    #[test]
    fn valid_artist_list() {
        assert!(ArtistList::is_valid("水瀬いのり"));
        assert!(ArtistList::is_valid("ArtistA（MemberB、MemberC）"));
        assert!(ArtistList::is_valid("ArtistA（MemberB、MemberC、SubArtistD（MemberE、MemberF））"));
        assert!(ArtistList::is_valid("ArtistA（MemberB、MemberC、SubArtistD（MemberE、MemberF））、ArtistG"));
    }

    #[test]
    fn invalid_artist_list() {
        assert!(!ArtistList::is_valid("水瀬いのり、"));
        assert!(!ArtistList::is_valid("、水瀬いのり"));
        assert!(!ArtistList::is_valid("水瀬いのり（"));
        assert!(!ArtistList::is_valid("水瀬いのり）"));
        assert!(!ArtistList::is_valid("水瀬いのり））"));
        assert!(!ArtistList::is_valid("（水瀬いのり"));
        assert!(!ArtistList::is_valid("水瀬いのり（）"));
        assert!(!ArtistList::is_valid("ArtistA（MemberB、MemberC、）"));
        assert!(!ArtistList::is_valid("ArtistA（MemberB、MemberC）ArtistB"));
        assert!(!ArtistList::is_valid("ArtistA（MemberB、MemberC"));
        assert!(!ArtistList::is_valid("ArtistA（MemberB、MemberC））"));
        assert!(!ArtistList::is_valid("ArtistA（SubArtistD（MemberE、MemberF）"));
        assert!(!ArtistList::is_valid("ArtistA（SubArtistD（MemberE、MemberF）））"));
    }

    #[test]
    fn deserialize_artist() {
        let input = "Petit Rabbit's（ココア（佐倉綾音）、チノ（水瀬いのり）、リゼ（種田梨沙）、千夜（佐藤聡美）、シャロ（内田真礼））";
        let artist = AnniArtist::new(input.to_owned()).unwrap();
        let list = artist.to_artist_list();
        assert_eq!(input, format!("{}", artist));
        assert_eq!(format!("{}", artist), format!("{}", list));
    }
}

