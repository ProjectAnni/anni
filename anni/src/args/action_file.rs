use std::convert::Infallible;
use std::fs::File;
use std::io::{stdin, stdout, Read, Write};
use std::str::FromStr;

/// ActionFile for file input or output
///
/// ActionFile can be used to create `FileReader` with `to_reader`
/// method or to `FileWriter` with `to_writer` method.
#[derive(Debug, Clone)]
pub struct ActionFile(String);

impl FromStr for ActionFile {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(ActionFile(s.to_string()))
    }
}

impl ActionFile {
    /// Create a `FileReader` from `ActionFile`
    pub fn to_reader(&self) -> anyhow::Result<Box<dyn Read + '_>> {
        if self.0 == "-" {
            // open stdin
            Ok(Box::new(stdin().lock()))
        } else {
            // open file
            Ok(Box::new(File::open(&self.0)?))
        }
    }

    /// Create a `FileWriter` from `ActionFile`
    pub fn to_writer(&self) -> anyhow::Result<Box<dyn Write + '_>> {
        if self.0 == "-" {
            // open stdout
            Ok(Box::new(stdout().lock()))
        } else {
            // open file
            Ok(Box::new(File::create(&self.0)?))
        }
    }
}
