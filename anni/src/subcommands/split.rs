use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use clap::{Clap, ArgEnum};

use anni_common::fs;
use anni_common::decode::{DecodeError, u16_le, u32_le, token};
use anni_common::encode::{btoken_w, u16_le_w, u32_le_w};

use anni_common::traits::{Decode, Encode};
use anni_derive::ClapHandler;
use anni_flac::{FlacHeader, MetadataBlock, MetadataBlockData};
use anni_flac::blocks::{UserComment, UserCommentExt, BlockPicture, PictureType};
use cue_sheet::tracklist::Tracklist;
use crate::{ll, ball};
use std::fmt::{Display, Formatter};

#[derive(Clap, ClapHandler, Debug)]
#[clap(about = ll ! ("split"))]
#[clap_handler(handle_split)]
pub struct SplitSubcommand {
    #[clap(arg_enum)]
    #[clap(short, long, default_value = "wav")]
    #[clap(about = ll ! {"split-format-input"})]
    input_format: SplitFormat,

    #[clap(arg_enum)]
    #[clap(short, long, default_value = "flac")]
    #[clap(about = ll ! {"split-format-output"})]
    output_format: SplitOutputFormat,

    #[clap(long = "no-apply-tags", parse(from_flag = std::ops::Not::not))]
    #[clap(about = ll ! {"split-no-apply-tags"})]
    apply_tags: bool,

    #[clap(long = "no-import-cover", parse(from_flag = std::ops::Not::not))]
    #[clap(about = ll ! {"split-no-import-cover"})]
    import_cover: bool,

    directory: PathBuf,
}

impl SplitSubcommand {
    fn split<P>(&self, audio_path: P, cue_path: P, cover: Option<P>) -> anyhow::Result<()>
        where P: AsRef<Path> {
        info!(target: "split", "Splitting...");

        let mut input = self.input_format.to_process(audio_path.as_ref())?;
        let mut audio = input.get_reader();
        let audio = &mut audio;

        // read header first
        let mut header = WaveHeader::from_reader(audio)?;

        // extract cue break points
        let tracks = cue_tracks(cue_path.as_ref());
        struct TrackInfo {
            begin: usize,
            end: usize,
            name: String,
            tags: Vec<UserComment>,
        }

        // generate time points
        let mut time_points: Vec<_> = tracks.iter().map(|i| (&header).mmssff(i.mm, i.ss, i.ff)).collect();
        time_points.push(header.data_size as usize);

        // generate track info
        let tracks: Vec<_> = tracks.into_iter().enumerate().map(|(i, track)| TrackInfo {
            begin: time_points[i],
            end: time_points[i + 1],
            name: format!("{:02}. {}", track.index, track.title).replace("/", "／"),
            tags: track.tags,
        }).collect();

        // generate file names & check whether file exists before split
        let files = tracks.iter().map(|track| {
            let filename = format!("{}.{}", track.name, self.output_format);
            let output = cue_path.as_ref().with_file_name(&filename);
            // check if file exists
            if output.exists() /* TODO: && !override_file */ {
                ball!("split-output-file-exist", filename = filename);
            } else {
                // save file path
                Ok(output)
            }
        }).collect::<anyhow::Result<Vec<_>>>()?;

        // do split & write tags
        tracks.into_iter().zip(files).try_for_each(|(mut track, path)| -> anyhow::Result<()>{
            info!(target: "split", "{}...", track.name);

            // choose output format
            let mut process = self.output_format.to_process(&path)?;

            // split wav from start to end
            split_wav(&mut header, audio, &mut process.get_writer(), track.begin, track.end)?;

            // wait for process to exit
            process.wait();

            if self.apply_tags || cover.is_some() && matches!(self.output_format, SplitOutputFormat::Flac) {
                // info!(target: "split", "Writing tags...");
                let mut flac = FlacHeader::from_file(&path)?;

                // write tags
                if self.apply_tags {
                    let comment = flac.comments_mut();
                    comment.clear();
                    comment.comments.append(&mut track.tags);
                }

                // write cover
                if let Some(cover) = &cover {
                    let picture = BlockPicture::new(cover, PictureType::CoverFront, String::new())?;
                    flac.blocks.push(MetadataBlock::new(MetadataBlockData::Picture(picture)));
                }

                // save flac file
                flac.save(Some(path))?;
            }
            Ok(())
        })?;

        // TODO: remove input file

        info!(target: "split", "Finished!");
        Ok(())
    }
}

#[derive(ArgEnum, Debug)]
pub enum SplitFormat {
    Wav,
    Flac,
    Ape,
    Tak,
}

impl SplitFormat {
    fn as_str(&self) -> &'static str {
        match self {
            SplitFormat::Wav => "wav",
            SplitFormat::Flac => "flac",
            SplitFormat::Ape => "ape",
            SplitFormat::Tak => "tak",
        }
    }

    fn get_encoder(&self) -> anyhow::Result<PathBuf> {
        encoder_of(self.as_str())
    }

    fn to_process<P>(&self, path: P) -> anyhow::Result<FileProcess>
        where P: AsRef<Path> {
        match self {
            SplitFormat::Wav => Ok(FileProcess::File(File::open(path.as_ref())?)),
            SplitFormat::Flac => {
                let process = Command::new(self.get_encoder()?)
                    .args(&["-c", "-d"])
                    .arg(path.as_ref().as_os_str())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null()) // ignore flac log output
                    .spawn()?;
                Ok(FileProcess::Process(process))
            }
            SplitFormat::Ape => {
                let process = Command::new(self.get_encoder()?)
                    .arg(path.as_ref().as_os_str())
                    .args(&["-", "-d"])
                    .stdout(Stdio::piped())
                    .spawn()?;
                Ok(FileProcess::Process(process))
            }
            SplitFormat::Tak => {
                let process = Command::new(self.get_encoder()?)
                    .arg("-d")
                    .arg(path.as_ref().as_os_str())
                    .arg("-")
                    .stdout(Stdio::piped())
                    .spawn()?;
                Ok(FileProcess::Process(process))
            }
        }
    }
}

impl Display for SplitFormat {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(ArgEnum, Debug)]
enum SplitOutputFormat {
    Flac,
    Wav,
}

impl SplitOutputFormat {
    fn as_str(&self) -> &'static str {
        match self {
            SplitOutputFormat::Flac => "flac",
            SplitOutputFormat::Wav => "wav",
        }
    }

    fn get_encoder(&self) -> anyhow::Result<PathBuf> {
        encoder_of(self.as_str())
    }

    fn to_process<P>(&self, path: P) -> anyhow::Result<FileProcess>
        where P: AsRef<Path> {
        match self {
            SplitOutputFormat::Wav => Ok(FileProcess::File(File::create(path.as_ref())?)),
            SplitOutputFormat::Flac => {
                let process = Command::new(self.get_encoder()?)
                    .args(&["--totally-silent", "-", "-o"])
                    .arg(path.as_ref().as_os_str())
                    .stdin(Stdio::piped())
                    .spawn()?;
                Ok(FileProcess::Process(process))
            }
        }
    }
}

impl Display for SplitOutputFormat {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

fn handle_split(me: &SplitSubcommand) -> anyhow::Result<()> {
    let cue = fs::get_ext_file(me.directory.as_path(), "cue", false)?
        .ok_or_else(|| anyhow!("Failed to find CUE sheet."))?;
    let audio = fs::get_ext_file(me.directory.as_path(), me.input_format.as_str(), false)?
        .ok_or_else(|| anyhow!("Failed to find audio file."))?;

    // try to get cover
    let cover = if me.import_cover { fs::get_ext_file(me.directory.as_path(), "jpg", false)? } else { None };
    if me.import_cover && cover.is_none() {
        warn!(target: "split", "Cover not found!");
    }

    me.split(audio, cue, cover)
}

fn encoder_of(format: &str) -> anyhow::Result<PathBuf> {
    let encoder = match format {
        "flac" => "flac",
        "ape" => "mac",
        "tak" => "takc",
        "wav" => return Ok(PathBuf::new()),
        _ => bail!("unsupported format"),
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

enum FileProcess {
    File(File),
    Process(Child),
}

impl FileProcess {
    fn get_reader(&mut self) -> &mut dyn Read {
        match self {
            FileProcess::File(f) => f,
            FileProcess::Process(p) => p.stdout.as_mut().unwrap(),
        }
    }

    fn get_writer(&mut self) -> &mut dyn Write {
        match self {
            FileProcess::File(f) => f,
            FileProcess::Process(p) => p.stdin.as_mut().unwrap(),
        }
    }

    fn wait(&mut self) {
        if let FileProcess::Process(p) = self {
            let ret = p.wait().unwrap();
            if !ret.success() {
                error!("Encoding process returned {}", ret.code().unwrap())
            }
        }
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

    // remove REM COMMENT
    let rem_comment = regex::Regex::new(r#"(?m)^\s*REM COMMENT .+$"#).unwrap();
    let cue = rem_comment.replace_all(&cue, "");

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
