// use std::io::{Read, Write};
// use std::process::{Command, Stdio};
//
// pub trait Decoder {
//     // TODO: progress should have better type
//     fn decode(
//         &self,
//         input: impl Read,
//         output: impl Write,
//         progress: Option<impl Write>,
//     ) -> Result<(), Error>;
// }
//
// pub struct CommandDecoder {
//     command: String,
//     arguments: Vec<String>,
// }
//
// impl CommandDecoder {
//     pub const INPUT_FILE_PLACEHOLDER: &'static str = "/*__ANNI_SPLIT_INPUT_FILE_PLACEHOLDER__*/";
//
//     pub fn new(command: String, arguments: Vec<String>) -> Self {
//         Self { command, arguments }
//     }
// }
//
// impl Decoder for CommandDecoder {
//     fn decode(
//         &self,
//         mut input: impl Read,
//         mut output: impl Write,
//         mut progress: Option<impl Write>,
//     ) -> Result<(), Error> {
//         let process = Command::new(&self.command)
//             .args(&self.arguments)
//             .stdin(Stdio::piped())
//             .stdout(Stdio::piped())
//             .stderr(if progress.is_some() {
//                 Stdio::piped()
//             } else {
//                 Stdio::null()
//             })
//             .spawn()?;
//
//         std::io::copy(&mut input, &mut process.stdin.unwrap())?;
//         std::io::copy()
//
//         Ok(())
//     }
// }
