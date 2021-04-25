use crate::subcommands::Subcommand;
use clap::{App, ArgMatches, Arg};
use std::io::{Write, Read};
use anni_common::{Decode, Encode};
use anni_utils::{decode, fs};
use anni_utils::decode::{u32_le, u16_le, DecodeError};
use std::fs::File;
use anni_utils::encode::{btoken_w, u32_le_w, u16_le_w};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio, Child};
use crate::i18n::ClapI18n;

pub struct SplitSubcommand;

impl Subcommand for SplitSubcommand {
    fn name(&self) -> &'static str {
        "split"
    }

    fn create(&self) -> App<'static> {
        App::new("split")
            .about_ll("split")
            .arg(Arg::new("split.format.input")
                .about_ll("split-format-input")
                .long("input-format")
                .short('i')
                .takes_value(true)
                .default_value("wav")
                .possible_values(&["wav", "flac", "ape"])
            )
            .arg(Arg::new("split.format.output")
                .about_ll("split-format-output")
                .long("output-format")
                .short('o')
                .takes_value(true)
                .default_value("flac")
                .possible_values(&["wav", "flac"])
            )
            .arg(Arg::new("Directory")
                .required(true)
                .takes_value(true)
            )
    }

    fn handle(&self, matches: &ArgMatches) -> anyhow::Result<()> {
        let input_format = matches.value_of("split.format.input").unwrap();
        encoder_of(input_format)?;

        let output_format = matches.value_of("split.format.output").unwrap();
        if input_format != output_format {
            encoder_of(output_format)?;
        }

        if let Some(dir) = matches.value_of("Directory") {
            let path = PathBuf::from(dir);
            let cue = fs::get_ext_file(&path, "cue", false)?
                .ok_or(anyhow!("Failed to find CUE sheet."))?;
            let audio = fs::get_ext_file(&path, input_format, false)?
                .ok_or(anyhow!("Failed to find audio file."))?;

            SplitTask::new(audio, input_format, output_format)?
                .split(cue)?;
        }
        Ok(())
    }
}

fn encoder_of(format: &str) -> anyhow::Result<PathBuf> {
    let encoder = match format {
        "flac" => "flac",
        "ape" => "mac",
        "wav" => return Ok(PathBuf::new()),
        _ => unimplemented!(),
    };
    let path = which::which(encoder)?;
    Ok(path)
}

#[derive(Debug)]
pub struct WaveHeader {
    pub channels: u16,
    pub sample_rate: u32,
    pub byte_rate: u32,
    pub block_align: u16,
    pub bit_per_sample: u16,
    pub data_size: u32,
}

impl Decode for WaveHeader {
    type Err = DecodeError;

    fn from_reader<R: Read>(reader: &mut R) -> Result<Self, Self::Err> {
        // RIFF chunk
        decode::token(reader, b"RIFF")?;
        let _chunk_size = u32_le(reader)?;
        debug!("RIFF chunk detected, size = {size}", size = _chunk_size);
        decode::token(reader, b"WAVE")?;

        // fmt sub-chunk
        decode::token(reader, b"fmt ")?;
        let _fmt_size = u32_le(reader)?;
        debug!("Chunk [fmt ] found, size = {size}", size = _fmt_size);

        let audio_format = u16_le(reader)?;
        if audio_format != 1 {
            error!("Only PCM format(1) is supported for now, got {}", audio_format);
            return Err(DecodeError::InvalidTokenError { expected: b"1".to_vec(), got: vec![(audio_format >> 8) as u8, (audio_format & 0xff) as u8] });
        }

        let channels = u16_le(reader)?;
        let sample_rate = u32_le(reader)?;
        let byte_rate = u32_le(reader)?;
        let block_align = u16_le(reader)?;
        let bit_per_sample = u16_le(reader)?;
        debug!("  channles = {}", channels);
        debug!("  sample_rate = {}", sample_rate);
        debug!("  byte_rate = {}", byte_rate);
        debug!("  block_alibn = {}", block_align);
        debug!("  bit_per_sample = {}", bit_per_sample);

        // data sub-chunk
        decode::token(reader, b"data")?;
        let data_size = u32_le(reader)?;
        debug!("Chunk [data] found, size = {size}", size = data_size);
        Ok(WaveHeader {
            channels,
            sample_rate,
            byte_rate,
            block_align,
            bit_per_sample,
            data_size,
        })
    }
}

impl Encode for WaveHeader {
    type Err = std::io::Error;

    fn write_to<W: Write>(&self, writer: &mut W) -> Result<(), Self::Err> {
        btoken_w(writer, b"RIFF")?;
        u32_le_w(writer, self.data_size + 16)?; // chunk size
        btoken_w(writer, b"WAVE")?;
        btoken_w(writer, b"fmt ")?;
        u32_le_w(writer, 16)?; // PCM chunk size
        u16_le_w(writer, 1)?; // audio format = 1, PCM
        u16_le_w(writer, self.channels)?;
        u32_le_w(writer, self.sample_rate)?;
        u32_le_w(writer, self.byte_rate)?;
        u16_le_w(writer, self.block_align)?;
        u16_le_w(writer, self.bit_per_sample)?;
        btoken_w(writer, b"data")?;
        u32_le_w(writer, self.data_size)?;
        Ok(())
    }
}

impl WaveHeader {
    pub fn mmssff(&self, m: usize, s: usize, f: usize) -> usize {
        let br = self.byte_rate as usize;
        br * 60 * m + br * s + br * f / 75
    }
}

struct SplitTask<'a> {
    output_format: &'a str,
    input: FileProcess,
}

impl<'a> SplitTask<'a> {
    pub fn new(audio_path: PathBuf, input_format: &str, output_format: &'a str) -> anyhow::Result<Self> {
        let input = match input_format {
            "wav" => FileProcess::File(File::open(audio_path)?),
            "flac" => {
                let process = Command::new(encoder_of("flac").unwrap())
                    .args(&["-c", "-d"])
                    .arg(audio_path.into_os_string())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null()) // ignore flac log output
                    .spawn()?;
                FileProcess::Process(process)
            }
            "ape" => {
                let process = Command::new(encoder_of("ape").unwrap())
                    .arg(audio_path.into_os_string())
                    .args(&["-", "-d"])
                    .stdout(Stdio::piped())
                    .spawn()?;
                FileProcess::Process(process)
            }
            _ => unreachable!(),
        };
        Ok(Self { output_format, input })
    }

    pub fn split<P: AsRef<Path>>(&mut self, cue_path: P) -> anyhow::Result<()> {
        let mut audio = self.input.get_reader();
        let audio = &mut audio;

        // read header first
        let mut header = WaveHeader::from_reader(audio)?;

        // extract cue break points
        let mut tracks: Vec<(String, usize)> = crate::subcommands::cue::extract_breakpoints(cue_path.as_ref())
            .iter()
            .map(|i| (format!("{:02}. {}", i.index, i.title), (&header).mmssff(i.mm, i.ss, i.ff)))
            .collect();
        tracks.push((String::new(), header.data_size as usize));
        let mut track_iter = tracks.iter();
        eprintln!("Splitting...");

        let mut prev = track_iter.next().unwrap();
        let mut processes = Vec::with_capacity(tracks.len() - 1);
        for now in track_iter {
            eprintln!("{}...", prev.0);
            // split track with filename
            let output = cue_path.as_ref().with_file_name(format!("{}.{}", prev.0, self.output_format).replace("/", "／"));
            // output file exists
            if output.exists() {
                bail!("Output file exists! Please remove the file and try again!");
            }
            // choose output format
            let mut process = match self.output_format {
                "wav" => FileProcess::File(File::create(output)?),
                "flac" => {
                    let process = Command::new(encoder_of("flac").unwrap())
                        .args(&["--totally-silent", "-", "-o"])
                        .arg(output.into_os_string())
                        .stdin(Stdio::piped())
                        .spawn()?;
                    FileProcess::Process(process)
                }
                _ => unimplemented!(),
            };
            // split wav from start to end
            split_wav(&mut header, audio, &mut process.get_writer(), prev.1, now.1)?;
            processes.push(process);
            prev = now;
        }
        // wait for all processes
        for mut p in processes {
            p.wait();
        }
        eprintln!("Finished!");
        Ok(())
    }
}

enum FileProcess {
    File(File),
    Process(Child),
}

impl FileProcess {
    fn get_reader(&mut self) -> Box<&mut dyn Read> {
        match self {
            FileProcess::File(f) => Box::new(f),
            FileProcess::Process(p) => Box::new(p.stdout.as_mut().unwrap()),
        }
    }

    fn get_writer(&mut self) -> Box<&mut dyn Write> {
        match self {
            FileProcess::File(f) => Box::new(f),
            FileProcess::Process(p) => Box::new(p.stdin.as_mut().unwrap()),
        }
    }

    fn wait(&mut self) {
        match self {
            FileProcess::Process(p) => {
                let ret = p.wait().unwrap();
                if !ret.success() {
                    error!("Encoding process returned {}", ret.code().unwrap())
                }
            }
            _ => {}
        };
    }
}

fn split_wav<I: Read, O: Write>(header: &mut WaveHeader, input: &mut I, output: &mut O, start: usize, end: usize) -> anyhow::Result<()> {
    let size = end - start;
    header.data_size = size as u32;
    header.write_to(output)?;
    std::io::copy(&mut input.take(size as u64), output)?;
    Ok(())
}
