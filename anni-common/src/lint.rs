use crate::diagnostic::{Diagnostic, DiagnosticSeverity};

pub trait AnniLinter {
    fn add(&mut self, msg: Diagnostic);
    fn flush(&self) -> bool;
}

#[derive(Default)]
pub struct AnniLinterReviewDogJsonLineFormat(bool);

impl AnniLinter for AnniLinterReviewDogJsonLineFormat {
    fn add(&mut self, msg: Diagnostic) {
        if let DiagnosticSeverity::Error = msg.severity {
            self.0 = true;
        }
        println!("{}", serde_json::to_string(&msg).unwrap());
    }

    fn flush(&self) -> bool { !self.0 }
}

#[derive(Default)]
pub struct AnniLinterTextFormat {
    errors: Vec<Diagnostic>,
    warnings: Vec<Diagnostic>,
    info: Vec<Diagnostic>,
}

impl AnniLinter for AnniLinterTextFormat {
    fn add(&mut self, msg: Diagnostic) {
        match msg.severity {
            DiagnosticSeverity::Error => self.errors.push(msg),
            DiagnosticSeverity::Warning => self.warnings.push(msg),
            DiagnosticSeverity::Info => self.info.push(msg),
        }
    }

    fn flush(&self) -> bool {
        println!("{} errors, {} warnings", self.errors.len(), self.warnings.len());
        println!();
        for error in self.errors.iter() {
            println!("ERROR:{}:{}:{}: {}", error.location.path, error.location.start_line(), error.location.start_column().unwrap_or(0), error.message);
        }
        println!();
        for warn in self.warnings.iter() {
            println!("WARN:{}:{}:{}: {}", warn.location.path, warn.location.start_line(), warn.location.start_column().unwrap_or(0), warn.message);
        }

        return self.errors.is_empty();
    }
}