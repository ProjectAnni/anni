use std::fmt;

use thiserror::Error;

/// A protocol path relative to either the immutable source root or a job's
/// staging root.
///
/// The string is preserved byte-for-byte as UTF-8. In particular, this type
/// performs no Unicode normalization or punctuation replacement.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SafeRelativePath(Box<str>);

impl SafeRelativePath {
    pub fn new(path: impl Into<String>) -> Result<Self, PathError> {
        let path = path.into();
        if path.is_empty() {
            return Err(PathError::Empty);
        }
        if path.starts_with('/') {
            return Err(PathError::Absolute);
        }

        for (index, byte) in path.bytes().enumerate() {
            match byte {
                0 => return Err(PathError::NulByte { index }),
                b'\\' => return Err(PathError::Backslash { index }),
                _ => {}
            }
        }

        for (index, component) in path.split('/').enumerate() {
            match component {
                "" => return Err(PathError::EmptyComponent { index }),
                "." | ".." => {
                    return Err(PathError::TraversalComponent {
                        index,
                        component: component.to_owned(),
                    });
                }
                _ => {}
            }
        }

        Ok(Self(path.into_boxed_str()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SafeRelativePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl fmt::Debug for SafeRelativePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("SafeRelativePath")
            .field(&self.0)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PathError {
    #[error("relative path cannot be empty")]
    Empty,
    #[error("absolute paths are not allowed")]
    Absolute,
    #[error("relative path contains a NUL byte at offset {index}")]
    NulByte { index: usize },
    #[error("relative path contains a backslash at offset {index}; use '/' as the separator")]
    Backslash { index: usize },
    #[error("relative path contains an empty component at index {index}")]
    EmptyComponent { index: usize },
    #[error("relative path contains traversal component {component:?} at index {index}")]
    TraversalComponent { index: usize, component: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_preserves_unicode_but_rejects_ambiguous_or_traversing_inputs() {
        let original = "歌詞／画像/Ａ・Ｂ〜Ｃ（初回盤）.png";
        assert_eq!(SafeRelativePath::new(original).unwrap().as_str(), original);

        for invalid in ["", "/absolute.wav", "../track.wav", "a/./b", "a//b", "a\\b"] {
            assert!(SafeRelativePath::new(invalid).is_err(), "{invalid:?}");
        }
    }
}
