use std::io::Read;
use std::path::{Path, PathBuf};

use clap::{ArgAction, Args, ValueEnum};

use anni_common::fs;

use crate::{ball, ll};
use anni_flac::blocks::{BlockPicture, PictureType, UserComment, UserCommentExt};
use anni_flac::{FlacHeader, MetadataBlock, MetadataBlockData};
use anni_split::codec::wav::{WavDecoder, WavEncoder};
use anni_split::codec::{
    ApeCommandDecoder, Decoder, Encoder, FlacCommandDecoder, FlacCommandEncoder, TakCommandDecoder,
    TtaCommandDecoder,
};
use anni_split::error::SplitError;
use anni_split::{cue_breakpoints, split};
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

        let input = self
            .input_format
            .get_decoder(audio_path.as_ref().to_path_buf());
        let (breakpoints, cue) = cue_breakpoints(fs::read_to_string(cue_path.as_ref())?)?;
        let tracks = cue_tracks(cue);

        // generate file names & check whether file exists before split
        let files = tracks
            .iter()
            .map(|track| {
                let filename =
                    format!("{:02}. {}.{}", track.index, track.title, self.output_format)
                        .replace("/", "／");
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
        if !self.dry_run {
            split(
                input,
                |index| {
                    let file = files[index].as_path();
                    info!(target: "split", "{}...", file.file_name().unwrap().to_string_lossy());
                    Ok(self.output_format.get_encoder(file))
                },
                breakpoints,
            )?;

            // TODO: write metadata
            if !self.clean && matches!(self.output_format, SplitOutputFormat::Flac) {
                for (path, mut track) in files.into_iter().zip(tracks) {
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

            // Option to remove full track after successful split
            if self.need_remove_after_success() {
                debug!(target: "split", "Removing audio file: {}", audio_path.as_ref().display());
                fs::remove_file(audio_path, self.trashcan)?;
                debug!(target: "split", "Removing cue file: {}", cue_path.as_ref().display());
                fs::remove_file(cue_path, self.trashcan)?;
            }
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

pub enum SplitFormats<P>
where
    P: AsRef<Path>,
{
    Wav(WavDecoder<P>),
    Flac(FlacCommandDecoder<P>),
    Ape(ApeCommandDecoder<P>),
    Tak(TakCommandDecoder<P>),
    Tta(TtaCommandDecoder<P>),
}

impl<P> Decoder for SplitFormats<P>
where
    P: AsRef<Path> + 'static,
{
    type Output = Box<dyn Read + Send>;

    fn decode(self) -> Result<Self::Output, SplitError> {
        Ok(match self {
            SplitFormats::Wav(decoder) => Box::new(decoder.decode()?),
            SplitFormats::Flac(decoder) => Box::new(decoder.decode()?),
            SplitFormats::Ape(decoder) => Box::new(decoder.decode()?),
            SplitFormats::Tak(decoder) => Box::new(decoder.decode()?),
            SplitFormats::Tta(decoder) => Box::new(decoder.decode()?),
        })
    }
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

    fn get_decoder<P>(&self, path: P) -> SplitFormats<P>
    where
        P: AsRef<Path>,
    {
        match self {
            SplitFormat::Wav => SplitFormats::Wav(WavDecoder(path)),
            SplitFormat::Flac => SplitFormats::Flac(FlacCommandDecoder(path)),
            SplitFormat::Ape => SplitFormats::Ape(ApeCommandDecoder(path)),
            SplitFormat::Tak => SplitFormats::Tak(TakCommandDecoder(path)),
            SplitFormat::Tta => SplitFormats::Tta(TtaCommandDecoder(path)),
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

    fn get_encoder<P>(&self, path: P) -> SplitOutputFormats<P>
    where
        P: AsRef<Path>,
    {
        match self {
            SplitOutputFormat::Flac => SplitOutputFormats::Flac(FlacCommandEncoder(path)),
            SplitOutputFormat::Wav => SplitOutputFormats::Wav(WavEncoder(path)),
        }
    }
}

pub enum SplitOutputFormats<P>
where
    P: AsRef<Path>,
{
    Wav(WavEncoder<P>),
    Flac(FlacCommandEncoder<P>),
}

impl<P> Encoder for SplitOutputFormats<P>
where
    P: AsRef<Path>,
{
    fn encode(self, input: impl Read) -> Result<(), SplitError> {
        match self {
            SplitOutputFormats::Wav(encoder) => encoder.encode(input),
            SplitOutputFormats::Flac(encoder) => encoder.encode(input),
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

struct CueTrack {
    pub index: u8,
    pub title: String,
    pub tags: Vec<UserComment>,
}

fn cue_tracks(cue: Cuna) -> Vec<CueTrack> {
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
                    result.push(CueTrack {
                        index: (i + 1) as u8,
                        title: title.to_owned(),
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
