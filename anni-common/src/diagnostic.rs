// https://github.com/reviewdog/reviewdog/tree/master/proto/rdf

use serde::Serialize;

#[derive(Serialize)]
pub struct Diagnostic<T> {
    pub message: DiagnosticMessage<T>,
    pub location: DiagnosticLocation,
    pub severity: DiagnosticSeverity,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<DiagnosticSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<DiagnosticCode>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub suggestions: Vec<DiagnosticSuggestion>,
}

impl<T> Diagnostic<T> {
    pub fn error(message: DiagnosticMessage<T>, location: DiagnosticLocation) -> Self {
        Self {
            message,
            location,
            severity: DiagnosticSeverity::Error,

            source: None,
            code: None,
            suggestions: Vec::new(),
        }
    }

    pub fn warning(message: DiagnosticMessage<T>, location: DiagnosticLocation) -> Self {
        Self {
            message,
            location,
            severity: DiagnosticSeverity::Warning,

            source: None,
            code: None,
            suggestions: Vec::new(),
        }
    }
}

pub struct DiagnosticMessage<T> {
    pub target: T,
    pub message: String,
}

impl<T> Serialize for DiagnosticMessage<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
    {
        serializer.serialize_str(&self.message)
    }
}

// Serialize here may never be used, but we need this for compile
#[derive(Clone, Serialize)]
pub enum MetadataDiagnosticTarget {
    Identifier(String, Option<u8>, Option<u8>),
    Tag(String),
}

impl MetadataDiagnosticTarget {
    pub fn album(album_id: String) -> Self {
        Self::Identifier(album_id, None, None)
    }

    pub fn disc(album_id: String, disc_id: u8) -> Self {
        Self::Identifier(album_id, Some(disc_id), None)
    }

    pub fn track(album_id: String, disc_id: u8, track_id: u8) -> Self {
        Self::Identifier(album_id, Some(disc_id), Some(track_id))
    }
}

#[derive(Serialize)]
pub struct DiagnosticLocation {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<DiagnosticRange>,
}

impl DiagnosticLocation {
    pub fn simple(path: String) -> DiagnosticLocation {
        DiagnosticLocation { path, range: None }
    }
}

impl DiagnosticLocation {
    pub fn start_line(&self) -> u32 {
        self.range.as_ref().map(|r| r.start.line).unwrap_or(0)
    }

    pub fn start_column(&self) -> Option<u32> {
        self.range.as_ref().and_then(|r| r.start.column)
    }

    pub fn end_line(&self) -> Option<u32> {
        self.range.as_ref().and_then(|r| r.end.as_ref().and_then(|p| Some(p.line)))
    }

    pub fn end_column(&self) -> Option<u32> {
        self.range.as_ref().and_then(|r| r.end.as_ref().and_then(|p| p.column))
    }
}

#[derive(Serialize, Default)]
pub struct DiagnosticRange {
    pub start: DiagnosticPosition,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<DiagnosticPosition>,
}

#[derive(Serialize, Default)]
pub struct DiagnosticPosition {
    pub line: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
}

#[derive(Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    // should be Info
    #[serde(rename = "INFO")]
    Information,
    // does not exist in rdf
    Hint,
}

impl Default for DiagnosticSeverity {
    fn default() -> Self {
        DiagnosticSeverity::Information
    }
}

#[derive(Serialize, Default)]
pub struct DiagnosticSource {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Serialize, Default)]
pub struct DiagnosticCode {
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

impl DiagnosticCode {
    pub fn new(value: String) -> Self {
        Self { value, ..Default::default() }
    }
}

#[derive(Serialize, Default)]
pub struct DiagnosticSuggestion {
    pub range: DiagnosticRange,
    pub text: String,
}
