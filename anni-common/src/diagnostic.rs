// https://github.com/reviewdog/reviewdog/tree/master/proto/rdf

use serde::Serialize;

#[derive(Serialize, Default)]
pub struct Diagnostic {
    pub message: String,
    pub location: DiagnosticLocation,
    pub severity: DiagnosticSeverity,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<DiagnosticSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<DiagnosticCode>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub suggestions: Vec<DiagnosticSuggestion>,
}

#[derive(Serialize, Default)]
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
#[serde(rename_all = "lowercase")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
}

impl Default for DiagnosticSeverity {
    fn default() -> Self {
        DiagnosticSeverity::Info
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
