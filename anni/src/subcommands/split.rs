use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use clap::{ArgAction, Args, ValueEnum};

use anni_common::decode::{token, u16_le, u32_le, DecodeError};
use anni_common::encode::{btoken_w, u16_le_w, u32_le_w};
use anni_common::fs;

use crate::{ball, ll};
use anni_common::traits::{Decode, Encode};
use anni_flac::blocks::{BlockPicture, PictureType, UserComment, UserCommentExt};
use anni_flac::{FlacHeader, MetadataBlock, MetadataBlockData};
use clap_handler::handler;
use cuna::Cuna;
use std::fmt::{Display, Formatter};

#[derive(Args, Debug, Clone)]
#[clap(about = ll!("split"))]
pub struct SplitSubcommand {
    #[clap(value_enum)]
    #[clap(short, long, default_value = "wav")]
    #[clap(help = ll!("split-format-input"))]
    input_format: SplitFormat,

    #[clap(value_enum)]
    #[clap(short, long, default_value = "flac")]
    #[clap(help = ll!("split-format-output"))]
    output_format: SplitOutputFormat,

    #[clap(long = "clean")]
    #[clap(help = ll!("split-clean"))]
    clean: bool,

    #[clap(long = "no-import-cover", action = ArgAction::SetFalse, default_value_t = true)]
    #[clap(help = ll!("split-no-import-cover"))]
    import_cover: bool,

    #[clap(long = "keep", action = ArgAction::SetFalse, default_value_t = true)]
    remove_after_success: bool,

    #[clap(long = "no-trashcan", action = ArgAction::SetFalse, default_value_t = true)]
    trashcan: bool,

    #[clap(long = "dry-run")]
    dry_run: bool,

    directories: Vec<PathBuf>,
}

impl SplitSubcommand {
    fn need_remove_after_success(&self) -> bool {
        !self.dry_run && self.remove_after_success
    }

    fn split<P>(&self, audio_path: P, cue_path: P, cover: Option<P>) -> anyhow::Result<()>
    where
        P: AsRef<Path>,
    {
        info!(target: "split", "Splitting {}...", audio_path.as_ref().display());

        let mut input = self.input_format.to_process(audio_path.as_ref())?;
        let mut audio = input.get_reader();
        let audio = &mut audio;

        // read header first
        let mut header = WaveHeader::from_reader(audio)?;

        // extract cue break points
        let tracks = cue_tracks(cue_path.as_ref());
        struct TrackInfo {
            begin: u32,
            end: u32,
            name: String,
            tags: Vec<UserComment>,
        }

        // generate time points
        let mut time_points: Vec<_> = tracks
            .iter()
            .map(|i| (&header).offset_from_second_frames(i.seconds, i.frames))
            .collect();
        time_points.push(header.data_size);

        // generate track info
        let tracks: Vec<_> = tracks
            .into_iter()
            .enumerate()
            .map(|(i, track)| TrackInfo {
                begin: time_points[i],
                end: time_points[i + 1],
                name: format!("{:02}. {}", track.index, track.title).replace("/", "／"),
                tags: track.tags,
            })
            .collect();

        // generate file names & check whether file exists before split
        let files = tracks
            .iter()
            .map(|track| {
                let filename = format!("{}.{}", track.name, self.output_format);
                // file output path is relative to cue path
                let output = cue_path.as_ref().with_file_name(&filename);
                // check if file exists
                if output.exists()
                /* TODO: && !override_file */
                {
                    ball!("split-output-file-exist", filename = filename);
                } else {
                    // save file path
                    Ok(output)
                }
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        // do split & write tags
        tracks
            .into_iter()
            .zip(files)
            .try_for_each(|(mut track, path)| -> anyhow::Result<()> {
                info!(target: "split", "{}...", track.name);

                if !self.dry_run {
                    // choose output format
                    let mut process = self.output_format.to_process(&path)?;

                    // split wav from start to end
                    split_wav(
                        &mut header,
                        audio,
                        &mut process.get_writer(),
                        track.begin,
                        track.end,
                    )?;

                    // wait for process to exit
                    process.wait();

                    if !self.clean && matches!(self.output_format, SplitOutputFormat::Flac) {
                        let mut flac = FlacHeader::from_file(&path)?;

                        // write tags
                        let comment = flac.comments_mut();
                        comment.clear();
                        comment.comments.append(&mut track.tags);

                        // write cover
                        if let Some(cover) = &cover {
                            let picture =
                                BlockPicture::new(cover, PictureType::CoverFront, String::new())?;
                            flac.blocks
                                .push(MetadataBlock::new(MetadataBlockData::Picture(picture)));
                        }

                        // save flac file
                        flac.save(Some(path))?;
                    }
                }
                Ok(())
            })?;

        // Option to remove full track after successful split
        if self.need_remove_after_success() {
            debug!(target: "split", "Removing audio file: {}", audio_path.as_ref().display());
            fs::remove_file(audio_path, self.trashcan)?;
            debug!(target: "split", "Removing cue file: {}", cue_path.as_ref().display());
            fs::remove_file(cue_path, self.trashcan)?;
        }

        Ok(())
    }
}

#[derive(ValueEnum, Debug, Clone)]
pub enum SplitFormat {
    Wav,
    Flac,
    Ape,
    Tak,
    Tta,
}

impl SplitFormat {
    fn as_str(&self) -> &str {
        match self {
            SplitFormat::Wav => "wav",
            SplitFormat::Flac => "flac",
            SplitFormat::Ape => "ape",
            SplitFormat::Tak => "tak",
            SplitFormat::Tta => "tta",
        }
    }

    fn get_encoder(&self) -> anyhow::Result<PathBuf> {
        encoder_of(self.as_str())
    }

    fn to_process<P>(&self, path: P) -> anyhow::Result<FileProcess>
    where
        P: AsRef<Path>,
    {
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
            SplitFormat::Tta => {
                let process = Command::new(self.get_encoder()?)
                    .args(&["-d", "-o", "-"])
                    .arg(path.as_ref().as_os_str())
                    .stdout(Stdio::piped())
                    .spawn()?;
                Ok(FileProcess::Process(process))
            }
        }
    }

    fn check_decoder(&self) -> bool {
        match self {
            SplitFormat::Wav => true,
            SplitFormat::Flac => match which::which("flac") {
                Ok(path) => {
                    debug!(target: "split", "FLAC decoder detected at: {}", path.display());
                    true
                }
                _ => false,
            },
            SplitFormat::Ape => match which::which("mac") {
                Ok(path) => {
                    debug!(target: "split", "APE decoder detected at: {}", path.display());
                    true
                }
                _ => false,
            },
            SplitFormat::Tak => match which::which("takc") {
                Ok(path) => {
                    debug!(target: "split", "TAK decoder detected at: {}", path.display());
                    true
                }
                _ => false,
            },
            SplitFormat::Tta => match which::which("ttaenc") {
                Ok(path) => {
                    debug!(target: "split", "TTA decoder detected at: {}", path.display());
                    true
                }
                _ => false,
            },
        }
    }
}

impl Display for SplitFormat {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(ValueEnum, Debug, Clone)]
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
    where
        P: AsRef<Path>,
    {
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

    fn check_encoder(&self) -> bool {
        match self {
            SplitOutputFormat::Wav => true,
            SplitOutputFormat::Flac => match which::which("flac") {
                Ok(path) => {
                    debug!(target: "split", "FLAC encoder detected at: {}", path.display());
                    true
                }
                _ => false,
            },
        }
    }
}

impl Display for SplitOutputFormat {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

fn get_cover(root: &PathBuf) -> anyhow::Result<Option<PathBuf>> {
    if let Some(cover) = fs::get_ext_file(root.as_path(), "jpg", false)? {
        let mut file = fs::File::open(&cover)?;
        let mut buffer = [0u8; 3];
        file.read_exact(&mut buffer)?;
        if buffer == [255, 216, 255] {
            return Ok(Some(cover));
        } else {
            log::warn!(
                "Cover file {} is not a JPEG file: Expected FF D8 FF, got {:2X} {:2X} {:2X}",
                cover.display(),
                buffer[0],
                buffer[1],
                buffer[2]
            );
        }
    }
    Ok(None)
}

#[handler(SplitSubcommand)]
fn handle_split(me: &SplitSubcommand) -> anyhow::Result<()> {
    if !me.input_format.check_decoder() || !me.output_format.check_encoder() {
        bail!("Some of the required encoders/decoders are missing. Please install them and try again.");
    }

    for directory in me.directories.iter() {
        if !directory.is_dir() {
            warn!(target: "split", "Ignoring non-dir file {}", directory.display());
            continue;
        }

        let audio = fs::get_ext_file(directory.as_path(), me.input_format.as_str(), false)?
            .ok_or_else(|| {
                anyhow!(
                    "Failed to find audio file from directory {}",
                    directory.display()
                )
            })?;
        let cue = {
            let audio_cue = audio.with_extension("cue");
            if audio_cue.is_file() {
                audio_cue
            } else {
                fs::get_ext_file(directory.as_path(), "cue", false)?.ok_or_else(|| {
                    anyhow!(
                        "Failed to find cue file from directory {}",
                        directory.display()
                    )
                })?
            }
        };

        // try to get cover
        let cover = if me.import_cover {
            get_cover(directory)?
        } else {
            None
        };
        if me.import_cover && cover.is_none() {
            warn!(target: "split", "Cover not found in directory {}", directory.display());
        }

        me.split(audio, cue, cover)?;
    }

    // log 'Finished' after all tracks were split
    info!(target: "split", "Finished!");
    Ok(())
}

fn encoder_of(format: &str) -> anyhow::Result<PathBuf> {
    let encoder = match format {
        "flac" => "flac",
        "ape" => "mac",
        "tak" => "takc",
        "tta" => "ttaenc",
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
            error!(
                "Only PCM format(1) is supported for now, got {}",
                audio_format
            );
            return Err(DecodeError::InvalidTokenError {
                expected: b"1".to_vec(),
                got: vec![(audio_format >> 8) as u8, (audio_format & 0xff) as u8],
            });
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
    pub fn offset_from_second_frames(&self, s: u32, f: u32) -> u32 {
        let br = self.byte_rate;
        br * s + br * f / 75
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

fn split_wav<I: Read, O: Write>(
    header: &mut WaveHeader,
    input: &mut I,
    output: &mut O,
    start: u32,
    end: u32,
) -> anyhow::Result<()> {
    let size = end - start;
    header.data_size = size;
    header.write_to(output)?;
    std::io::copy(&mut input.take(size as u64), output)?;
    Ok(())
}

struct CueTrack {
    pub index: u8,
    pub title: String,
    pub seconds: u32,
    pub frames: u32,
    pub tags: Vec<UserComment>,
}

fn cue_tracks<P: AsRef<Path>>(path: P) -> Vec<CueTrack> {
    let cue = anni_common::fs::read_to_string(path).unwrap();
    let cue = Cuna::new(&cue).unwrap();
    debug!("{:#?}", cue);

    let album = cue.title().get(0).map(String::as_str).unwrap_or("");
    let artist = cue.performer().get(0).map(String::as_str).unwrap_or("");

    let mut track_number = 1;
    let track_total = cue.files.iter().map(|f| f.tracks.len()).sum();

    let mut result = Vec::with_capacity(track_total);
    for file in cue.files.iter() {
        for (i, track) in file.tracks.iter().enumerate() {
            for index in track.index.iter() {
                if index.id() == 1 {
                    let title = track
                        .title
                        .get(0)
                        .map(String::to_owned)
                        .unwrap_or(format!("Track {}", track_number));
                    let artist = track.performer.get(0).map(String::as_str).unwrap_or(artist);
                    let time = index.begin_time();
                    result.push(CueTrack {
                        index: (i + 1) as u8,
                        title: title.to_owned(),
                        seconds: time.total_seconds(),
                        frames: time.frames(),
                        tags: vec![
                            UserComment::title(title),
                            UserComment::album(album),
                            UserComment::artist(artist),
                            UserComment::track_number(track_number),
                            UserComment::track_total(track_total),
                        ],
                    });
                }
            }
            track_number += 1;
        }
    }
    result
}
