use crate::decoder::Decoder;
use crate::error::SplitError;
use std::ffi::OsStr;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};

/// Placeholder for [CommandDecoder] to indicate the input file path.
pub const INPUT_FILE_PLACEHOLDER: &str = "/*__ANNI_SPLIT_INPUT_FILE_PLACEHOLDER__*/";

/// [CommandDecoder] is a [Decoder] that spawns a external command to do the decoding.
///
/// Use [INPUT_FILE_PLACEHOLDER] to indicate the input path. It would be replaced with the actual input file path.
pub struct CommandDecoder<C, A>
where
    C: AsRef<OsStr>,
    A: IntoIterator<Item = C>,
{
    command: C,
    arguments: A,
}

impl<C, A> CommandDecoder<C, A>
where
    C: AsRef<OsStr>,
    A: IntoIterator<Item = C>,
{
    pub fn new(command: C, arguments: A) -> Self {
        Self { command, arguments }
    }
}

impl<C, A> Decoder for CommandDecoder<C, A>
where
    C: AsRef<OsStr>,
    A: IntoIterator<Item = C>,
{
    type Output = impl Read;

    fn decode(self, mut input: impl Read + Send + 'static) -> Result<Self::Output, SplitError> {
        let mut process = Command::new(self.command)
            .args(self.arguments)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;

        let mut stdin = process.stdin.take().unwrap();
        let stdout = process.stdout.take().unwrap();

        std::thread::spawn(move || {
            std::io::copy(&mut input, &mut stdin).unwrap();
        });

        Ok(stdout)
    }

    fn decode_file<P>(self, input: P) -> Result<Self::Output, SplitError>
    where
        P: AsRef<Path>,
    {
        let args = self.arguments.into_iter().collect::<Vec<_>>();
        let args = args.iter().map(|arg| {
            if arg.as_ref() == INPUT_FILE_PLACEHOLDER {
                input.as_ref().as_os_str()
            } else {
                arg.as_ref()
            }
        });

        let mut process = Command::new(self.command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;

        let stdout = process.stdout.take().unwrap();

        Ok(stdout)
    }
}
