use crate::diagnostic::{Diagnostic, DiagnosticSeverity};
use serde::Serialize;

pub trait AnniLinter<T> {
    fn add(&mut self, msg: Diagnostic<T>);
    fn flush(&self) -> bool;
}

#[derive(Default)]
pub struct AnniLinterReviewDogJsonLineFormat(bool);

impl AnniLinterReviewDogJsonLineFormat {
    pub fn new() -> Self {
        AnniLinterReviewDogJsonLineFormat(false)
    }
}

impl<T: Serialize> AnniLinter<T> for AnniLinterReviewDogJsonLineFormat {
    fn add(&mut self, msg: Diagnostic<T>) {
        if let DiagnosticSeverity::Error = msg.severity {
            self.0 = true;
        }
        println!("{}", serde_json::to_string(&msg).unwrap());
    }

    fn flush(&self) -> bool {
        !self.0
    }
}

pub struct AnniLinterTextFormat<T> {
    errors: Vec<Diagnostic<T>>,
    warnings: Vec<Diagnostic<T>>,
}

impl<T> Default for AnniLinterTextFormat<T> {
    fn default() -> Self {
        Self {
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }
}

impl<T> AnniLinter<T> for AnniLinterTextFormat<T> {
    fn add(&mut self, msg: Diagnostic<T>) {
        match msg.severity {
            DiagnosticSeverity::Error => self.errors.push(msg),
            DiagnosticSeverity::Warning => self.warnings.push(msg),
            _ => {}
        }
    }

    fn flush(&self) -> bool {
        println!(
            "{} errors, {} warnings",
            self.errors.len(),
            self.warnings.len()
        );
        println!();
        for error in self.errors.iter() {
            println!(
                "[ERROR][{}] {}:{}: {}",
                error.location.path,
                error.location.start_line(),
                error.location.start_column().unwrap_or(0),
                error.message.message
            );
        }
        println!();
        for warn in self.warnings.iter() {
            println!(
                "[WARN][{}] {}:{}: {}",
                warn.location.path,
                warn.location.start_line(),
                warn.location.start_column().unwrap_or(0),
                warn.message.message
            );
        }

        self.errors.is_empty()
    }
}
