use prettytable::{Table, Row, Cell, Attr, color};
use term::stderr;
use std::io;

pub trait Reporter {
    fn add_problem(&mut self, field: &str, problem: &str, key: &str, value: Option<&str>, suggestion: Option<&str>);
    fn print(&self);
    fn eprint(&self);
}

pub fn new(mode: &str) -> Box<dyn Reporter> {
    match mode {
        "table" => Box::new(TableReporter::default()),
        "markdown" => Box::new(MarkdownReporter::default()),
        _ => panic!("Invalid reporter mode."),
    }
}

pub struct MarkdownReporter {
    len: usize,
    text: String,
}

impl Reporter for MarkdownReporter {
    fn add_problem(&mut self, field: &str, problem: &str, key: &str, value: Option<&str>, suggestion: Option<&str>) {
        if self.len == 0 {
            self.text += "## File ";
            self.text += field;
            self.text += ":\n";
        }
        let value = match value {
            Some(str) => format!("={}", str),
            None => "".to_owned(),
        };
        let suggestion = match suggestion {
            Some(str) => format!("({})", str),
            None => "".to_owned(),
        };
        self.text += &format!("- {}: {}{} {}\n", problem, key, value, suggestion);
    }

    fn print(&self) {
        if !self.text.is_empty() {
            print!("{}", self.text);
        }
    }

    fn eprint(&self) {
        if !self.text.is_empty() {
            eprint!("{}", self.text);
        }
    }
}

impl Default for MarkdownReporter {
    fn default() -> Self {
        Self { len: 0, text: String::new() }
    }
}

pub struct TableReporter {
    table: Table,
}

impl Reporter for TableReporter {
    fn add_problem(&mut self, field: &str, problem: &str, key: &str, value: Option<&str>, suggestion: Option<&str>) {
        let field = if self.table.len() == 1 { field } else { "" };
        let value = value.unwrap_or("-");
        let suggestion = suggestion.unwrap_or("");
        self.table.add_row(Row::new(vec![
            Cell::new(field),
            Cell::new(problem).with_style(Attr::ForegroundColor(color::RED)),
            Cell::new(key).with_style(Attr::Bold),
            Cell::new(value),
            Cell::new(suggestion).with_style(Attr::ForegroundColor(color::GREEN)),
        ]));
    }

    fn print(&self) {
        if self.table.len() > 1 {
            self.table.printstd();
        }
    }

    fn eprint(&self) {
        if self.table.len() > 1 {
            let r = match (stderr(), true) {
                (Some(mut o), true) => self.table.print_term(&mut *o),
                _ => self.table.print(&mut io::stderr()),
            };
            r.expect("Failed to print to stderr.");
        }
    }
}

impl Default for TableReporter {
    fn default() -> Self {
        let mut table = Table::new();
        table.add_row(Row::new(vec![
            Cell::new("Field").with_style(Attr::Bold).with_style(Attr::ForegroundColor(color::BRIGHT_WHITE)),
            Cell::new("Problem").with_style(Attr::Bold).with_style(Attr::ForegroundColor(color::BRIGHT_WHITE)),
            Cell::new("Key").with_style(Attr::Bold).with_style(Attr::ForegroundColor(color::BRIGHT_WHITE)),
            Cell::new("Value").with_style(Attr::Bold).with_style(Attr::ForegroundColor(color::BRIGHT_WHITE)),
            Cell::new("Suggestion").with_style(Attr::Bold).with_style(Attr::ForegroundColor(color::BRIGHT_WHITE)),
        ]));
        Self { table }
    }
}