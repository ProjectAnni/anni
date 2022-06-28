use crate::args::{FlacInputFile, InputPath};
use crate::ll;
use anni_flac::blocks::PictureType;
use anni_flac::{FlacHeader, MetadataBlockData};
use clap::{ArgEnum, Args, Subcommand};
use clap_handler::{handler, Handler};
use std::io::Write;

#[derive(Args, Handler, Debug, Clone)]
#[clap(about = ll!("flac"))]
pub struct FlacSubcommand {
    #[clap(subcommand)]
    action: FlacAction,
}

#[derive(Subcommand, Handler, Debug, Clone)]
pub enum FlacAction {
    #[clap(about = ll!("flac-export"))]
    Export(FlacExportAction),
    RemoveID3(FlacRemoveID3Action),
}

#[derive(Args, Debug, Clone)]
pub struct FlacExportAction {
    #[clap(arg_enum)]
    #[clap(short = 't', long = "type", default_value = "tag")]
    #[clap(help = ll!{"flac-export-type"})]
    export_type: FlacExportType,

    #[clap(short = 'n', long)]
    // #[clap(about = ll!{"flac-export-block-num"})]
    block_num: Option<u8>,

    #[clap(long, default_value = "cover")]
    picture_type: PictureType,

    #[clap(short, long, default_value = "-")]
    #[clap(help = ll!{"export-to"})]
    output: crate::args::ActionFile,

    #[clap(required = true)]
    filename: Vec<InputPath<FlacInputFile>>,
}

#[handler(FlacExportAction)]
fn flac_export(me: &FlacExportAction) -> anyhow::Result<()> {
    for path in me.filename.iter() {
        for file in path.iter() {
            let stream = FlacHeader::from_file(file)?;
            me.export(&stream)?;
        }
    }
    Ok(())
}

impl FlacExportAction {
    fn export(&self, stream: &FlacHeader) -> anyhow::Result<()> {
        match self.export_type {
            FlacExportType::Info => self.export_inner(stream, "STREAMINFO"),
            FlacExportType::Application => self.export_inner(stream, "APPLICATION"),
            FlacExportType::Seektable => self.export_inner(stream, "SEEKTABLE"),
            FlacExportType::Cue => self.export_inner(stream, "CUESHEET"),
            FlacExportType::Comment => self.export_inner(stream, "VORBIS_COMMENT"),
            FlacExportType::Picture => self.export_inner(stream, "PICTURE"),
            FlacExportType::List => {
                for (i, block) in stream.blocks.iter().enumerate() {
                    let mut out = self.output.to_writer()?;
                    block.write(&mut out, i)?;
                }
                Ok(())
            }
        }
    }

    fn export_inner(&self, header: &FlacHeader, export_block_name: &str) -> anyhow::Result<()> {
        let mut first_picture = true;
        let mut out = self.output.to_writer()?;

        for (i, block) in header.blocks.iter().enumerate() {
            // if block_num is specified, only dump the specified type
            if let Some(block_num) = self.block_num {
                if block_num != i as u8 {
                    return Ok(());
                }
            }

            if block.data.as_str() == export_block_name {
                match &block.data {
                    MetadataBlockData::Comment(s) => write!(out, "{}", s)?,
                    // TODO
                    // MetadataBlockData::CueSheet(_) => {}
                    MetadataBlockData::Picture(p) => {
                        // only dump the first picture of specified type
                        if first_picture && p.picture_type == self.picture_type {
                            out.write_all(&p.data)?;
                            first_picture = false;
                        }
                    }
                    _ => block.write(&mut out, i)?,
                };
            }
        }
        Ok(())
    }
}

#[derive(ArgEnum, Debug, PartialEq, Clone)]
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

#[derive(Args, Debug, Clone)]
pub struct FlacRemoveID3Action {
    #[clap(required = true)]
    filename: Vec<InputPath<FlacInputFile>>,
}

#[handler(FlacRemoveID3Action)]
fn flac_remove_id3(me: &FlacRemoveID3Action) -> anyhow::Result<()> {
    for filenames in me.filename.iter() {
        for path in filenames.iter() {
            debug!("Opening {}", path.display());
            let mut file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&path)?;
            let removed = id3::Tag::remove_from_file(&mut file)?;
            if removed {
                info!("Removed ID3 tag from {}", path.display());
            } else {
                info!("No ID3 tag found in {}", path.display());
            }
        }
    }
    Ok(())
}
