use std::io::Write;
use std::path::{PathBuf, Path};
use anni_flac::blocks::PictureType;
use anni_flac::{MetadataBlockData, FlacHeader};
use clap::{Clap, ArgEnum};
use anni_common::fs;
use crate::ll;
use crate::cli::HandleArgs;

#[derive(Clap, Debug)]
#[clap(about = ll ! ("flac"))]
pub struct FlacSubcommand {
    #[clap(subcommand)]
    action: FlacAction,
}

impl HandleArgs for FlacSubcommand {
    fn handle(&self) -> anyhow::Result<()> {
        self.action.handle()
    }
}

#[derive(Clap, Debug)]
pub enum FlacAction {
    #[clap(about = ll ! ("flac-export"))]
    Export(FlacExportAction)
}

impl HandleArgs for FlacAction {
    fn handle(&self) -> anyhow::Result<()> {
        match self {
            FlacAction::Export(a) => a.handle()
        }
    }
}

#[derive(Clap, Debug)]
pub struct FlacExportAction {
    #[clap(arg_enum)]
    #[clap(short = 't', long = "type", default_value = "tag")]
    #[clap(about = ll ! {"flac-export-type"})]
    export_type: FlacExportType,

    #[clap(short = 'n', long)]
    // #[clap(about = ll ! {"flac-export-block-num"})]
    block_num: Option<u8>,

    #[clap(long, default_value = "cover")]
    picture_type: PictureType,

    #[clap(short, long, default_value = "-")]
    #[clap(about = ll ! {"flac-export-to"})]
    output: crate::args::ActionFile,
    #[clap(required = true)]
    filename: Vec<PathBuf>,
}

impl HandleArgs for FlacExportAction {
    fn handle(&self) -> anyhow::Result<()> {
        for path in self.filename.iter() {
            for (_, stream) in parse_input_iter(path) {
                let stream = stream?;
                self.export(&stream)?;
            }
        }
        Ok(())
    }
}

impl FlacExportAction {
    fn export(&self, stream: &FlacHeader) -> anyhow::Result<()> {
        match self.export_type {
            FlacExportType::Info => self.export_inner(&stream, "STREAMINFO"),
            FlacExportType::Application => self.export_inner(&stream, "APPLICATION"),
            FlacExportType::Seektable => self.export_inner(&stream, "SEEKTABLE"),
            FlacExportType::Cue => self.export_inner(&stream, "CUESHEET"),
            FlacExportType::Comment => self.export_inner(&stream, "VORBIS_COMMENT"),
            FlacExportType::Picture => self.export_inner(&stream, "PICTURE"),
            FlacExportType::List => {
                for (i, block) in stream.blocks.iter().enumerate() {
                    let mut out = self.output.to_writer()?;
                    let mut handle = out.lock();
                    block.write(&mut handle, i)?;
                }
                Ok(())
            }
        }
    }

    fn export_inner(&self, header: &FlacHeader, export_block_name: &str) -> anyhow::Result<()> {
        let mut first_picture = true;
        let mut out = self.output.to_writer()?;
        let mut handle = out.lock();

        for (i, block) in header.blocks.iter().enumerate() {
            // if block_num is specified, only dump the specified type
            if let Some(block_num) = self.block_num {
                if block_num != i as u8 {
                    return Ok(());
                }
            }

            if block.data.as_str() == export_block_name {
                match &block.data {
                    MetadataBlockData::Comment(s) => write!(handle, "{}", s)?,
                    // TODO
                    // MetadataBlockData::CueSheet(_) => {}
                    MetadataBlockData::Picture(p) => {
                        // only dump the first picture of specified type
                        if first_picture && p.picture_type == self.picture_type {
                            handle.write_all(&p.data)?;
                            first_picture = false;
                        }
                    }
                    _ => block.write(&mut handle, i)?,
                };
            }
        }
        Ok(())
    }
}

#[derive(ArgEnum, Debug, PartialEq)]
pub enum FlacExportType {
    /// Block Info
    Info,
    /// Block Application
    Application,
    /// Block Seektable
    Seektable,
    /// Block Cue
    Cue,
    /// Block Comment
    #[clap(alias = "tag")]
    Comment,
    /// Block Picture
    Picture,
    /// List All
    #[clap(alias = "all")]
    List,
}

/// Parse <path> and get all flac files within <path>
pub(crate) fn parse_input_iter<P: AsRef<Path>>(input: P) -> impl Iterator<Item=(PathBuf, anni_flac::prelude::Result<FlacHeader>)> {
    fs::PathWalker::new(input, true).filter_map(|file| {
        match file.extension() {
            None => return None,
            Some(ext) => {
                if ext != "flac" {
                    return None;
                }
            }
        };

        let header = FlacHeader::from_file(&file);
        Some((file, header))
    })
}
