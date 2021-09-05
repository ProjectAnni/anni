use std::str::FromStr;
use std::convert::Infallible;
use std::io::{stdin, Read, Stdin, Stdout, Write, stdout};
use std::fs::File;

/// ActionFile for file input or output
///
/// ActionFile can be used to create `FileReader` with `to_reader`
/// method or to `FileWriter` with `to_writer` method.
#[derive(Debug)]
pub struct ActionFile(String);

impl FromStr for ActionFile {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(ActionFile(s.to_string()))
    }
}

impl ActionFile {
    /// Create a `FileReader` from `ActionFile`
    pub fn to_reader(&self) -> anyhow::Result<FileReader> {
        if self.0 == "-" {
            // open stdin
            Ok(FileReader::Stdin(stdin()))
        } else {
            // open file
            Ok(FileReader::File(File::open(&self.0)?))
        }
    }

    /// Create a `FileWriter` from `ActionFile`
    pub fn to_writer(&self) -> anyhow::Result<FileWriter> {
        if self.0 == "-" {
            // open stdout
            Ok(FileWriter::Stdout(stdout()))
        } else {
            // open file
            Ok(FileWriter::File(File::create(&self.0)?))
        }
    }
}

/// FileReader to read data from custom file types
pub enum FileReader {
    Stdin(Stdin),
    File(File),
}

impl FileReader {
    pub fn lock(&mut self) -> Box<dyn Read + '_> {
        match self {
            FileReader::Stdin(stdin) => Box::new(stdin.lock()),
            FileReader::File(file) => Box::new(file),
        }
    }
}

/// FileWriter to write data to custom file types
pub enum FileWriter {
    Stdout(Stdout),
    File(File),
}

impl FileWriter {
    pub fn lock(&mut self) -> Box<dyn Write + '_> {
        match self {
            FileWriter::Stdout(stdout) => Box::new(stdout.lock()),
            FileWriter::File(file) => Box::new(file),
        }
    }
}
