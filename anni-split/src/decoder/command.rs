use crate::decoder::Decoder;
use crate::error::SplitError;
use std::ffi::OsStr;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use which::which;

/// Placeholder for [CommandDecoder] to indicate the input file path.
pub const INPUT_FILE_PLACEHOLDER: &str = "/*__ANNI_SPLIT_INPUT_FILE_PLACEHOLDER__*/";

/// [CommandDecoder] is a [Decoder] that spawns a external command to do the decoding.
///
/// Use [INPUT_FILE_PLACEHOLDER] to indicate the input path. It would be replaced with the actual input file path.
pub struct CommandDecoder<Cmd, Arg, Args>
where
    Cmd: AsRef<OsStr>,
    Arg: AsRef<OsStr>,
    Args: IntoIterator<Item = Arg>,
{
    command: Cmd,
    arguments: Args,
}

impl<Arg, Args> CommandDecoder<PathBuf, Arg, Args>
where
    Arg: AsRef<OsStr>,
    Args: IntoIterator<Item = Arg>,
{
    pub fn new<O>(command: O, arguments: Args) -> Result<Self, SplitError>
    where
        O: AsRef<OsStr>,
    {
        let command: PathBuf = which(command)?;

        Ok(Self { command, arguments })
    }
}

impl<Cmd, Arg, Args> Decoder for CommandDecoder<Cmd, Arg, Args>
where
    Cmd: AsRef<OsStr>,
    Arg: AsRef<OsStr>,
    Args: IntoIterator<Item = Arg>,
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
