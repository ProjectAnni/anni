use std::{
    collections::HashMap,
    ffi::OsString,
    fs::{self, File},
    io::{self, ErrorKind, Write},
    path::{Path, PathBuf},
};

use anni_common::models::RawTrackIdentifier;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
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
use symphonia_core::io::MediaSource;
use thiserror::Error;

use crate::{
    sources::cached_http::provider::AudioVariant,
    stats::{CacheStats, CacheStatsHandle},
    CODEC_REGISTRY,
};

#[derive(Debug, Clone)]
pub struct CacheStore {
    base: PathBuf,
    stats: CacheStatsHandle,
}

pub enum CacheAcquire {
    Hit(File),
    Miss { reader: File, writer: CacheWriter },
}

pub struct CacheWriter {
    file: Option<File>,
    lock: File,
    temporary_path: PathBuf,
    final_path: PathBuf,
    metadata_path: PathBuf,
    stats: CacheStatsHandle,
    finished: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct CacheMetadata {
    complete: bool,
    content_length: Option<u64>,
}

impl CacheStore {
    pub fn new(base: PathBuf) -> Self {
        Self {
            base,
            stats: CacheStatsHandle::default(),
        }
    }

    pub fn stats(&self) -> CacheStats {
        self.stats.snapshot()
    }

    pub(crate) fn open_variant_if_complete(
        &self,
        track: RawTrackIdentifier<'_>,
        variant: AudioVariant,
    ) -> io::Result<Option<File>> {
        let path = self.location_of_variant(track, variant);
        if self.is_complete_entry(&path)? {
            self.stats.hit();
            return File::open(path).map(Some);
        }
        Ok(None)
    }

    /// Legacy location without an audio variant. New code should use
    /// `location_of_variant` so encoded representations never alias.
    pub fn location_of(&self, track: RawTrackIdentifier<'_>) -> PathBuf {
        let mut path = self.base.clone();
        path.extend([
            track.album_id.as_ref(),
            &format!("{}_{}", track.disc_id, track.track_id),
        ]);
        path
    }

    #[deprecated(note = "use location_of")]
    pub fn loaction_of(&self, track: RawTrackIdentifier<'_>) -> PathBuf {
        self.location_of(track)
    }

    pub fn location_of_variant(
        &self,
        track: RawTrackIdentifier<'_>,
        variant: AudioVariant,
    ) -> PathBuf {
        let path = self.location_of(track);
        append_suffix(&path, &format!(".{}", variant.cache_suffix()))
    }

    pub fn acquire_variant(
        &self,
        track: RawTrackIdentifier<'_>,
        variant: AudioVariant,
    ) -> io::Result<CacheAcquire> {
        let path = self.location_of_variant(track.copied(), variant);
        create_dir_all(path.parent().expect("cache entry always has a parent"))?;

        let lock_path = append_suffix(&path, ".lock");
        let lock = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock_path)?;
        // File locking is provided directly by std::fs::File on the pinned
        // Rust toolchain. UFCS keeps that explicit and avoids implying that an
        // extension trait or external locking crate is required.
        File::lock(&lock)?;

        if self.is_complete_entry(&path)? {
            self.stats.hit();
            return Ok(CacheAcquire::Hit(File::open(path)?));
        }

        if path.exists() {
            fs::remove_file(&path)?;
        }
        let temporary_path = append_suffix(&path, ".part");
        let metadata_path = append_suffix(&path, ".info");
        if metadata_path.exists() {
            fs::remove_file(&metadata_path)?;
        }
        File::options()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&temporary_path)?;
        let reader = File::open(&temporary_path)?;
        let writer = File::options().append(true).open(&temporary_path)?;
        self.stats.start_download();

        Ok(CacheAcquire::Miss {
            reader,
            writer: CacheWriter {
                file: Some(writer),
                lock,
                temporary_path,
                final_path: path,
                metadata_path,
                stats: self.stats.clone(),
                finished: false,
            },
        })
    }

    fn is_complete_entry(&self, path: &Path) -> io::Result<bool> {
        if !path.exists() {
            return Ok(false);
        }

        let metadata_path = append_suffix(path, ".info");
        let metadata = File::open(metadata_path)
            .ok()
            .and_then(|file| serde_json::from_reader::<_, CacheMetadata>(file).ok());
        if let Some(metadata) = metadata
            && metadata.complete
        {
            let length_matches = metadata.content_length.is_none_or(|length| {
                File::open(path)
                    .and_then(|file| file.metadata())
                    .map(|metadata| metadata.len())
                    .is_ok_and(|actual| actual == length)
            });
            return Ok(length_matches);
        }

        Ok(validate_audio(path).unwrap_or(false))
    }

    /// Backwards-compatible cache API using the historical, representation-free key.
    pub fn acquire(&self, track: RawTrackIdentifier<'_>) -> io::Result<Result<File, (File, File)>> {
        let path = self.location_of(track.copied());
        if path.exists() && validate_audio(&path).unwrap_or(false) {
            return Ok(Ok(File::open(path)?));
        }
        create_dir_all(path.parent().expect("cache entry always has a parent"))?;
        File::options()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&path)?;
        Ok(Err((
            File::open(&path)?,
            File::options().append(true).open(path)?,
        )))
    }

    pub fn add(&self, source: &Path, track: RawTrackIdentifier<'_>) -> io::Result<()> {
        let location = self.location_of(track);
        create_dir_all(location.parent().expect("cache entry always has a parent"))?;
        if location.exists() {
            return Err(ErrorKind::AlreadyExists.into());
        }
        if !validate_audio(source).unwrap_or(false) {
            return Err(io::Error::other("invalid cache"));
        }
        fs::copy(source, location).map(|_| ())
    }

    pub fn add_variant(
        &self,
        source: &Path,
        track: RawTrackIdentifier<'_>,
        variant: AudioVariant,
    ) -> io::Result<()> {
        let location = self.location_of_variant(track, variant);
        create_dir_all(location.parent().expect("cache entry always has a parent"))?;
        if location.exists() {
            if validate_audio(&location).unwrap_or(false) {
                return Err(ErrorKind::AlreadyExists.into());
            }
            fs::remove_file(&location)?;
        }
        if !validate_audio(source).unwrap_or(false) {
            return Err(io::Error::other("invalid cache"));
        }
        let length = fs::copy(source, &location)?;
        atomic_write_json(
            &append_suffix(&location, ".info"),
            &CacheMetadata {
                complete: true,
                content_length: Some(length),
            },
        )
    }

    pub fn store_info<S>(
        &self,
        track: RawTrackIdentifier<'_>,
        key: &str,
        value: S,
    ) -> io::Result<()>
    where
        S: Serialize,
    {
        let path = append_suffix(&self.location_of(track), ".info");
        write_info(path, key, value)
    }

    pub fn acquire_info<T: DeserializeOwned>(
        &self,
        track: RawTrackIdentifier<'_>,
        key: &str,
    ) -> io::Result<Option<T>> {
        let path = append_suffix(&self.location_of(track), ".info");
        read_info_value(path, key)
    }
}

impl Write for CacheWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.file.as_mut().expect("cache writer is open").write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.as_mut().expect("cache writer is open").flush()
    }
}

impl CacheWriter {
    pub fn record_downloaded(&self, bytes: usize) {
        self.stats.downloaded(bytes);
    }

    pub fn finish(mut self, content_length: Option<u64>) -> io::Result<()> {
        let file = self.file.take().expect("cache writer is open");
        file.sync_all()?;
        let actual_length = file.metadata()?.len();
        drop(file);

        if content_length.is_some_and(|expected| expected != actual_length) {
            return Err(io::Error::new(
                ErrorKind::UnexpectedEof,
                format!(
                    "downloaded cache length {actual_length} does not match expected {}",
                    content_length.unwrap()
                ),
            ));
        }

        if !validate_audio(&self.temporary_path).unwrap_or(false) {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                "downloaded audio did not pass cache validation",
            ));
        }

        fs::rename(&self.temporary_path, &self.final_path)?;
        let metadata = CacheMetadata {
            complete: true,
            content_length: Some(actual_length),
        };
        if let Err(error) = atomic_write_json(&self.metadata_path, &metadata) {
            // The validated audio file is the source of truth. Missing metadata
            // only makes a later cache lookup validate the file again.
            log::warn!(
                "failed to write cache metadata for {}: {error}",
                self.final_path.display()
            );
        }
        self.finished = true;
        self.stats.finish_download(true);
        let _ = File::unlock(&self.lock);
        Ok(())
    }
}

impl Drop for CacheWriter {
    fn drop(&mut self) {
        if !self.finished {
            self.stats.finish_download(false);
        }
        let _ = File::unlock(&self.lock);
    }
}

fn append_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut value: OsString = path.as_os_str().to_owned();
    value.push(suffix);
    value.into()
}

fn atomic_write_json(path: &Path, value: &impl Serialize) -> io::Result<()> {
    create_dir_all(path.parent().expect("metadata always has a parent"))?;
    let temporary = append_suffix(path, ".tmp");
    let mut writer = File::options()
        .write(true)
        .truncate(true)
        .create(true)
        .open(&temporary)?;
    serde_json::to_writer(&mut writer, value)?;
    writer.sync_all()?;
    drop(writer);
    match fs::rename(&temporary, path) {
        Ok(()) => Ok(()),
        Err(_) if path.exists() => {
            // Windows does not replace an existing destination. The audio
            // entry remains self-validating if a crash occurs in this gap.
            fs::remove_file(path)?;
            fs::rename(temporary, path)
        }
        Err(error) => Err(error),
    }
}

fn write_info<S: Serialize>(path: PathBuf, key: &str, value: S) -> io::Result<()> {
    let mut info = match File::open(&path) {
        Ok(file) => read_info(&file)?,
        Err(error) if error.kind() == ErrorKind::NotFound => HashMap::new(),
        Err(error) => return Err(error),
    };
    info.insert(key.to_owned(), serde_json::to_value(value)?);
    atomic_write_json(&path, &info)
}

fn read_info_value<T: DeserializeOwned>(path: PathBuf, key: &str) -> io::Result<Option<T>> {
    match File::open(path) {
        Ok(file) => Ok(read_info(&file)?
            .remove(key)
            .map(serde_json::from_value)
            .transpose()?),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn read_info(file: &File) -> serde_json::Result<HashMap<String, Value>> {
    serde_json::from_reader(file)
}

pub fn create_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
    fs::create_dir_all(path)
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
    let mut format_reader = get_probe().probe(
        &Hint::new(),
        source,
        FormatOptions::default(),
        MetadataOptions::default(),
    )?;
    let Some(track) = format_reader.default_track(TrackType::Audio) else {
        return Ok(false);
    };
    let track_id = track.id;
    let Some(codec_params) = track
        .codec_params
        .as_ref()
        .and_then(|params| params.audio())
    else {
        return Ok(false);
    };
    let mut decoder = CODEC_REGISTRY
        .make_audio_decoder(codec_params, &AudioDecoderOptions::default().verify(true))?;
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

pub fn validate_audio(path: &Path) -> symphonia::core::errors::Result<bool> {
    match validate(Box::new(File::open(path)?)) {
        Ok(result) => Ok(result),
        Err(ValidationError::Decode(error)) => Err(error),
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
    use std::{
        collections::VecDeque,
        convert::Infallible,
        fs,
        io::{Read, Write},
        num::NonZeroU8,
        time::{SystemTime, UNIX_EPOCH},
    };

    use anni_common::models::RawTrackIdentifier;
    use symphonia::core::{
        packet::Packet,
        units::{Duration, Timestamp},
    };

    use super::{append_suffix, for_each_packet_in_track, AudioVariant, CacheAcquire, CacheStore};
    use crate::sources::cached_http::provider::{AudioCodec, AudioQuality};

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

    #[test]
    fn cache_paths_include_codec_and_quality() {
        let store = CacheStore::new("cache".into());
        let track = RawTrackIdentifier::new(
            "album",
            NonZeroU8::new(1).unwrap(),
            NonZeroU8::new(2).unwrap(),
        );
        let low_opus = store.location_of_variant(
            track.copied(),
            AudioVariant::new(AudioQuality::Low, AudioCodec::Opus),
        );
        let high_aac = store.location_of_variant(
            track,
            AudioVariant::new(AudioQuality::High, AudioCodec::Aac),
        );
        assert_ne!(low_opus, high_aac);
        assert!(low_opus.to_string_lossy().ends_with("1_2.low-opus"));
    }

    #[test]
    fn completed_variant_is_published_as_a_cache_hit() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!("anni-playback-cache-{unique}"));
        let store = CacheStore::new(base.clone());
        let track = RawTrackIdentifier::new(
            "album",
            NonZeroU8::new(1).unwrap(),
            NonZeroU8::new(2).unwrap(),
        );
        let variant = AudioVariant::new(AudioQuality::Low, AudioCodec::Opus);

        match store.acquire_variant(track.copied(), variant).unwrap() {
            CacheAcquire::Miss { reader, mut writer } => {
                let audio = include_bytes!("../../../../assets/1s.flac");
                writer.write_all(audio).unwrap();
                writer.record_downloaded(audio.len());
                writer.finish(Some(audio.len() as u64)).unwrap();
                drop(reader);
            }
            CacheAcquire::Hit(_) => panic!("fresh cache should miss"),
        }

        match store.acquire_variant(track, variant).unwrap() {
            CacheAcquire::Hit(mut reader) => {
                let mut bytes = Vec::new();
                reader.read_to_end(&mut bytes).unwrap();
                assert_eq!(bytes, include_bytes!("../../../../assets/1s.flac"));
            }
            CacheAcquire::Miss { .. } => panic!("completed cache should hit"),
        }
        assert_eq!(store.stats().hits, 1);
        assert_eq!(store.stats().misses, 1);
        assert_eq!(store.stats().completed_downloads, 1);
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn invalid_download_is_never_published() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!("anni-playback-invalid-cache-{unique}"));
        let store = CacheStore::new(base.clone());
        let track = RawTrackIdentifier::new(
            "album",
            NonZeroU8::new(1).unwrap(),
            NonZeroU8::new(2).unwrap(),
        );
        let variant = AudioVariant::new(AudioQuality::High, AudioCodec::Aac);
        let final_path = store.location_of_variant(track.copied(), variant);

        match store.acquire_variant(track, variant).unwrap() {
            CacheAcquire::Miss { reader, mut writer } => {
                writer.write_all(b"not audio").unwrap();
                assert!(writer.finish(Some(9)).is_err());
                drop(reader);
            }
            CacheAcquire::Hit(_) => panic!("fresh cache should miss"),
        }

        assert!(!final_path.exists());
        assert_eq!(store.stats().failed_downloads, 1);
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn metadata_failure_does_not_fail_a_valid_download() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!("anni-playback-metadata-cache-{unique}"));
        let store = CacheStore::new(base.clone());
        let track = RawTrackIdentifier::new(
            "album",
            NonZeroU8::new(1).unwrap(),
            NonZeroU8::new(2).unwrap(),
        );
        let variant = AudioVariant::new(AudioQuality::Lossless, AudioCodec::Original);
        let final_path = store.location_of_variant(track.copied(), variant);
        let metadata_temporary_path = append_suffix(&append_suffix(&final_path, ".info"), ".tmp");

        match store.acquire_variant(track.copied(), variant).unwrap() {
            CacheAcquire::Miss { reader, mut writer } => {
                let audio = include_bytes!("../../../../assets/1s.flac");
                writer.write_all(audio).unwrap();
                writer.record_downloaded(audio.len());
                fs::create_dir_all(&metadata_temporary_path).unwrap();
                writer.finish(Some(audio.len() as u64)).unwrap();
                drop(reader);
            }
            CacheAcquire::Hit(_) => panic!("fresh cache should miss"),
        }

        assert!(final_path.exists());
        assert_eq!(store.stats().completed_downloads, 1);
        assert_eq!(store.stats().failed_downloads, 0);
        assert!(matches!(
            store.acquire_variant(track, variant).unwrap(),
            CacheAcquire::Hit(_)
        ));
        fs::remove_dir_all(base).unwrap();
    }
}
