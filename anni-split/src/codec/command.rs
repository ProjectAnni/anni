use crate::codec::{Decoder, Encoder};
use crate::error::SplitError;
use std::ffi::OsStr;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use which::which;

/// Placeholder for [CommandCodec] to indicate the input file path.
pub const FILE_PLACEHOLDER: &str = "/*__ANNI_SPLIT_COMMAND_CODEC_FILE_PLACEHOLDER__*/";

/// [CommandCodec] is a [Encoder] or [Decoder] that spawns a external command to do the en/decoding.
///
/// Use [FILE_PLACEHOLDER] to indicate the input path. It would be replaced with the actual input/output file path.
pub struct CommandCodec<Cmd, Arg, Args>
where
    Cmd: AsRef<OsStr>,
    Arg: AsRef<OsStr>,
    Args: IntoIterator<Item = Arg>,
{
    command: Cmd,
    arguments: Args,
}

impl<Arg, Args> CommandCodec<PathBuf, Arg, Args>
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

impl<Cmd, Arg, Args> Decoder for CommandCodec<Cmd, Arg, Args>
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
        let args = self
            .arguments
            .into_iter()
            .map(|arg| Replacer(arg, input.as_ref()));

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

impl<Cmd, Arg, Args> Encoder for CommandCodec<Cmd, Arg, Args>
where
    Cmd: AsRef<OsStr>,
    Arg: AsRef<OsStr>,
    Args: IntoIterator<Item = Arg>,
{
    fn encode<P>(self, mut input: impl Read + Send + 'static, output: P) -> Result<(), SplitError>
    where
        P: AsRef<Path>,
    {
        let args = self
            .arguments
            .into_iter()
            .map(|arg| Replacer(arg, output.as_ref()));

        let mut process = Command::new(self.command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        let mut stdin = process.stdin.take().unwrap();
        std::thread::spawn(move || {
            std::io::copy(&mut input, &mut stdin).unwrap();
        });

        process.wait()?;
        Ok(())
    }
}

/// Utility to reuse logic of [FILE_PLACEHOLDER] replacing.
struct Replacer<I, P>(I, P)
where
    I: AsRef<OsStr>,
    P: AsRef<Path>;

impl<I, P> AsRef<OsStr> for Replacer<I, P>
where
    I: AsRef<OsStr>,
    P: AsRef<Path>,
{
    fn as_ref(&self) -> &OsStr {
        if self.0.as_ref() == FILE_PLACEHOLDER {
            self.1.as_ref().as_os_str()
        } else {
            self.0.as_ref()
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::codec::command::{Replacer, FILE_PLACEHOLDER};

    #[test]
    fn test_replacer() {
        let path = "c";
        let args = ["a", "b", FILE_PLACEHOLDER, "d"];
        let mut result = args.into_iter().map(|a| Replacer(a, path));

        assert_eq!(result.next().unwrap().as_ref(), "a");
        assert_eq!(result.next().unwrap().as_ref(), "b");
        assert_eq!(result.next().unwrap().as_ref(), "c");
        assert_eq!(result.next().unwrap().as_ref(), "d");
        assert!(result.next().is_none());
    }
}
