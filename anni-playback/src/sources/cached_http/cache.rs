use std::{
    collections::HashMap,
    fs::{self, File},
    io::{self, ErrorKind},
    path::{Path, PathBuf},
};

use crate::CODEC_REGISTRY;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use symphonia::{
    core::{
        codecs::audio::AudioDecoderOptions,
        formats::{probe::Hint, FormatOptions, TrackType},
        io::MediaSourceStream,
        meta::MetadataOptions,
        packet::Packet,
    },
    default::get_probe,
};

use anni_common::models::RawTrackIdentifier;
use symphonia_core::io::MediaSource;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct CacheStore {
    base: PathBuf,
}

impl CacheStore {
    pub fn new(base: PathBuf) -> Self {
        Self { base }
    }

    /// Returns the path to given `track`
    pub fn loaction_of(&self, track: RawTrackIdentifier) -> PathBuf {
        let mut tmp = self.base.clone();

        tmp.extend([
            track.album_id.as_ref(),
            &format!(
                "{}_{}",
                track.disc_id.to_string(),
                track.track_id.to_string(),
            ),
        ]);
        tmp
    }

    /// Attempts to open a cache file corresponding to `track` and validates it.
    ///
    /// On success, returns a `Result<File, (File, File)>`.
    /// If the cache exists and is valid, opens it in read mode and returns an `Ok(_)`.
    /// Otherwise, creates or truncates a cache file, opens it in read mode as a `reader`
    /// and append mode as a `writer`, and returns an `Err((reader, writer))`
    ///
    /// On error, an [`Error`](std::io::Error) is returned.
    pub fn acquire(&self, track: RawTrackIdentifier) -> io::Result<Result<File, (File, File)>> {
        let path = self.loaction_of(track.copied());

        if path.exists() {
            let content_length = self.acquire_info::<u64>(track.copied(), "content-length")?;
            let f = File::open(&path)?;

            if content_length == Some(f.metadata()?.len()) || validate_audio(&path).unwrap_or(false)
            {
                return Ok(Ok(f));
            }

            log::warn!("cache of {track} exists but is invalid");
        }

        create_dir_all(path.parent().unwrap())?; // parent of `path` exists

        let _ = File::options()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&path)?; // truncate the file first to clear incorrect data

        let reader = File::options().read(true).open(&path)?;
        let writer = File::options().append(true).open(path)?;

        Ok(Err((reader, writer)))
    }

    pub fn add(&self, path: &Path, track: RawTrackIdentifier) -> io::Result<()> {
        let location = self.loaction_of(track);

        if location.exists() {
            Err(ErrorKind::AlreadyExists.into())
        } else if validate_audio(path).unwrap_or(false) {
            fs::copy(path, location).map(|_| {})
        } else {
            Err(io::Error::new(ErrorKind::Other, "invalid cache"))
        }
    }

    pub fn store_info<S>(&self, track: RawTrackIdentifier, key: &str, value: S) -> io::Result<()>
    where
        S: Serialize,
    {
        let path = {
            let mut p = self.loaction_of(track.copied());
            p.set_extension("info");
            p
        };

        let mut info = match File::open(&path) {
            Ok(f) => read_info(&f)?,
            Err(e) if e.kind() == ErrorKind::NotFound => HashMap::with_capacity(1),
            Err(e) => return Err(e.into()),
        };
        info.insert(key.to_owned(), serde_json::to_value(value)?);

        let writer = File::options()
            .write(true)
            .truncate(true)
            .create(true)
            .open(path)?;

        serde_json::to_writer(writer, &info)?;

        Ok(())
    }

    pub fn acquire_info<T: DeserializeOwned>(
        &self,
        track: RawTrackIdentifier,
        key: &str,
    ) -> io::Result<Option<T>> {
        let path = {
            let mut p = self.loaction_of(track);
            p.set_extension("info");
            p
        };

        match File::open(&path) {
            Ok(f) => Ok(read_info(&f)?
                .remove(key)
                .map(|v| serde_json::from_value(v))
                .transpose()?),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
            Err(e) => return Err(e),
        }
    }
}

fn read_info(f: &File) -> serde_json::Result<HashMap<String, Value>> {
    serde_json::from_reader(f)
}

pub fn create_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
    match fs::create_dir_all(path.as_ref()) {
        Err(e) if e.kind() == ErrorKind::AlreadyExists => Ok(()),
        r => r,
    }
}

fn for_each_packet_in_track<E>(
    track_id: u32,
    mut next_packet: impl FnMut() -> Result<Option<Packet>, E>,
    mut consume: impl FnMut(&Packet) -> Result<(), E>,
) -> Result<(), E> {
    while let Some(packet) = next_packet()? {
        if packet.track_id == track_id {
            consume(&packet)?;
        }
    }

    Ok(())
}

pub fn validate(source: Box<dyn MediaSource>) -> Result<bool, ValidationError> {
    let source = MediaSourceStream::new(source, Default::default());

    let format_opts = FormatOptions::default();
    let metadata_opts = MetadataOptions::default();

    let mut format_reader = get_probe().probe(&Hint::new(), source, format_opts, metadata_opts)?;
    let track = match format_reader.default_track(TrackType::Audio) {
        Some(track) => track,
        None => return Ok(false),
    };
    let track_id = track.id;
    let codec_params = match track
        .codec_params
        .as_ref()
        .and_then(|params| params.audio())
    {
        Some(params) => params,
        None => return Ok(false),
    };

    let options = AudioDecoderOptions::default().verify(true);
    let mut decoder = CODEC_REGISTRY.make_audio_decoder(codec_params, &options)?;

    for_each_packet_in_track(
        track_id,
        || format_reader.next_packet(),
        |packet| decoder.decode(packet).map(|_| ()),
    )?;

    decoder
        .finalize()
        .verify_ok
        .ok_or(ValidationError::Unsupported)
}

pub fn validate_audio(p: &Path) -> symphonia::core::errors::Result<bool> {
    match validate(Box::new(File::open(p)?)) {
        Ok(res) => Ok(res),
        Err(ValidationError::Decode(e)) => Err(e),
        Err(ValidationError::Unsupported) => Ok(true),
    }
}

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Decode Error: {0}")]
    Decode(#[from] symphonia::core::errors::Error),
    #[error("Validation is not supported on the source")]
    Unsupported,
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, convert::Infallible};

    use symphonia::core::{
        packet::Packet,
        units::{Duration, Timestamp},
    };

    use super::for_each_packet_in_track;

    #[test]
    fn decodes_only_packets_from_the_selected_track() {
        let mut packets = VecDeque::from([
            Packet::new(1, Timestamp::ZERO, Duration::ZERO, []),
            Packet::new(2, Timestamp::ZERO, Duration::ZERO, []),
            Packet::new(1, Timestamp::ZERO, Duration::ZERO, []),
        ]);
        let mut decoded_track_ids = Vec::new();

        for_each_packet_in_track(
            1,
            || Ok::<_, Infallible>(packets.pop_front()),
            |packet| {
                decoded_track_ids.push(packet.track_id);
                Ok::<_, Infallible>(())
            },
        )
        .unwrap();

        assert_eq!(decoded_track_ids, [1, 1]);
    }
}
