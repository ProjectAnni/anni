use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use clap::{App, Arg, ArgMatches};

use anni_common::fs;
use anni_common::decode::{DecodeError, u16_le, u32_le, token};
use anni_common::encode::{btoken_w, u16_le_w, u32_le_w};

use crate::i18n::ClapI18n;
use crate::subcommands::Subcommand;
use anni_common::traits::{Decode, Encode};
use anni_flac::FlacHeader;
use anni_flac::blocks::{UserComment, UserCommentExt};
use cue_sheet::tracklist::Tracklist;

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
            .arg(Arg::new("split.tags.apply")
                .about_ll("split-apply-tags")
                .long("apply-tags")
                .short('t')
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
                .split(cue, matches.is_present("split.tags.apply"))?;
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
        token(reader, b"RIFF")?;
        let _chunk_size = u32_le(reader)?;
        debug!("RIFF chunk detected, size = {size}", size = _chunk_size);
        token(reader, b"WAVE")?;

        // fmt sub-chunk
        token(reader, b"fmt ")?;
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
        debug!("  channels = {}", channels);
        debug!("  sample_rate = {}", sample_rate);
        debug!("  byte_rate = {}", byte_rate);
        debug!("  block_align = {}", block_align);
        debug!("  bit_per_sample = {}", bit_per_sample);

        // data sub-chunk
        token(reader, b"data")?;
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

    pub fn split<P: AsRef<Path>>(&mut self, cue_path: P, write_tags: bool) -> anyhow::Result<()> {
        let mut audio = self.input.get_reader();
        let audio = &mut audio;

        // read header first
        let mut header = WaveHeader::from_reader(audio)?;

        // extract cue break points
        let mut tracks: Vec<(String, usize, Vec<UserComment>)> = cue_tracks(cue_path.as_ref())
            .into_iter()
            .map(|i| (format!("{:02}. {}", i.index, i.title), (&header).mmssff(i.mm, i.ss, i.ff), i.tags))
            .collect();
        tracks.push((String::new(), header.data_size as usize, Vec::new()));
        let mut track_iter = tracks.iter();
        eprintln!("Splitting...");

        let mut prev = track_iter.next().unwrap();
        let mut processes = Vec::with_capacity(tracks.len() - 1);
        let mut files = Vec::new();
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
                "wav" => FileProcess::File(File::create(&output)?),
                "flac" => {
                    let process = Command::new(encoder_of("flac").unwrap())
                        .args(&["--totally-silent", "-", "-o"])
                        .arg(output.clone().into_os_string())
                        .stdin(Stdio::piped())
                        .spawn()?;
                    FileProcess::Process(process)
                }
                _ => unimplemented!(),
            };
            // split wav from start to end
            split_wav(&mut header, audio, &mut process.get_writer(), prev.1, now.1)?;
            processes.push(process);
            files.push(output);
            prev = now;
        }
        // wait for all processes
        for mut p in processes {
            p.wait();
        }

        if write_tags {
            // Write tags
            eprintln!("Writing tags...");
            for ((_, _, mut tags), path) in tracks.into_iter().zip(files) {
                let mut flac = FlacHeader::from_file(&path)?;
                let comment = flac.comments_mut();
                comment.clear();
                comment.comments.append(&mut tags);
                flac.save(Some(path))?;
            }
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

struct CueTrack {
    pub index: u8,
    pub title: String,
    pub mm: usize,
    pub ss: usize,
    pub ff: usize,
    pub tags: Vec<UserComment>,
}

fn cue_tracks<P: AsRef<Path>>(path: P) -> Vec<CueTrack> {
    let cue = anni_common::fs::read_to_string(path).unwrap();
    let cue = Tracklist::parse(&cue).unwrap();
    let album = cue.info.get("TITLE").map(String::as_str).unwrap_or("");
    let artist = cue.info.get("ARTIST").map(String::as_str).unwrap_or("");
    let date = cue.info.get("DATE").map(String::as_str).unwrap_or("");
    let disc_number = cue.info.get("DISCNUMBER").map(String::as_str).unwrap_or("1");
    let disc_total = cue.info.get("TOTALDISCS").map(String::as_str).unwrap_or("1");

    let mut track_number = 1;
    let track_total = cue.files.iter().map(|f| f.tracks.len()).sum();

    let mut result = Vec::with_capacity(track_total);
    for file in cue.files.iter() {
        for (i, track) in file.tracks.iter().enumerate() {
            for (index, time) in track.index.iter() {
                if *index == 1 {
                    let title = track.info.get("TITLE").map(String::to_owned).unwrap_or(format!("Track {}", track_number));
                    result.push(CueTrack {
                        index: (i + 1) as u8,
                        title: title.to_owned(),
                        mm: time.minutes() as usize,
                        ss: time.seconds() as usize,
                        ff: time.frames() as usize,
                        tags: vec![
                            UserComment::title(title),
                            UserComment::album(album),
                            UserComment::artist(track.info.get("ARTIST").map(String::as_str).unwrap_or(artist)),
                            UserComment::date(date),
                            UserComment::track_number(track_number),
                            UserComment::track_total(track_total),
                            UserComment::disc_number(disc_number),
                            UserComment::disc_total(disc_total),
                        ],
                    });
                }
            }
            track_number += 1;
        }
    }
    result
}
