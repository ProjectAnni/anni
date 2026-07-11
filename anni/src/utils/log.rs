/// Modified from https://github.com/ProjectAnni/pretty-env-logger
use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};

use env_logger::fmt::{style::Style, Formatter};
use log::{Level, Record};

/// Formatter used in env_logger builder
///
/// Formats output with colored level
pub fn formatter(f: &mut Formatter, record: &Record) -> std::io::Result<()> {
    use std::io::Write;
    let target = record.target();
    let max_width = max_target_width(target);

    let level_style = f.default_level_style(record.level());
    let level = level_label(record.level());
    let target_style = Style::new().bold();
    let target = Padded {
        value: target,
        width: max_width,
    };

    writeln!(
        f,
        " {level_style}{level}{level_style:#} {target_style}{target}{target_style:#} > {}",
        record.args(),
    )
}

struct Padded<T> {
    value: T,
    width: usize,
}

impl<T: fmt::Display> fmt::Display for Padded<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{: <width$}", self.value, width = self.width)
    }
}

static MAX_MODULE_WIDTH: AtomicUsize = AtomicUsize::new(0);

fn max_target_width(target: &str) -> usize {
    let max_width = MAX_MODULE_WIDTH.load(Ordering::Relaxed);
    if max_width < target.len() {
        MAX_MODULE_WIDTH.store(target.len(), Ordering::Relaxed);
        target.len()
    } else {
        max_width
    }
}

fn level_label(level: Level) -> &'static str {
    match level {
        Level::Trace => "TRACE",
        Level::Debug => "DEBUG",
        Level::Info => "INFO ",
        Level::Warn => "WARN ",
        Level::Error => "ERROR",
    }
}
