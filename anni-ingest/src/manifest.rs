use sha2::{Digest as ShaDigest, Sha256};
use thiserror::Error;

use crate::{Digest, SafeRelativePath};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioFormat {
    Wav,
    Flac,
    Alac,
    Aac,
    Mp3,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputFileKind {
    Audio(AudioFormat),
    CueSheet,
    Booklet,
    CoverCandidate,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestEntry {
    path: SafeRelativePath,
    byte_length: u64,
    digest: Digest,
    kind: InputFileKind,
}

impl ManifestEntry {
    pub const fn new(
        path: SafeRelativePath,
        byte_length: u64,
        digest: Digest,
        kind: InputFileKind,
    ) -> Self {
        Self {
            path,
            byte_length,
            digest,
            kind,
        }
    }

    pub const fn path(&self) -> &SafeRelativePath {
        &self.path
    }

    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }

    pub const fn digest(&self) -> Digest {
        self.digest
    }

    pub const fn kind(&self) -> InputFileKind {
        self.kind
    }
}

/// Immutable inventory of every file visible to an ingest worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputManifest {
    entries: Vec<ManifestEntry>,
    digest: Digest,
}

impl InputManifest {
    pub fn new(mut entries: Vec<ManifestEntry>) -> Result<Self, ManifestError> {
        if entries.is_empty() {
            return Err(ManifestError::Empty);
        }

        entries.sort_by(|left, right| left.path.cmp(&right.path));
        for pair in entries.windows(2) {
            if pair[0].path == pair[1].path {
                return Err(ManifestError::DuplicatePath {
                    path: pair[0].path.clone(),
                });
            }
        }

        let digest = digest_entries(&entries);
        Ok(Self { entries, digest })
    }

    pub fn entries(&self) -> &[ManifestEntry] {
        &self.entries
    }

    pub const fn digest(&self) -> Digest {
        self.digest
    }

    pub fn entry(&self, path: &SafeRelativePath) -> Option<&ManifestEntry> {
        self.entries
            .binary_search_by(|entry| entry.path.cmp(path))
            .ok()
            .map(|index| &self.entries[index])
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ManifestError {
    #[error("input manifest cannot be empty")]
    Empty,
    #[error("input manifest contains duplicate path {path}")]
    DuplicatePath { path: SafeRelativePath },
}

fn digest_entries(entries: &[ManifestEntry]) -> Digest {
    let mut hasher = Sha256::new();
    hasher.update(b"anni-ingest-manifest-v1\0");
    hash_u64(&mut hasher, entries.len() as u64);
    for entry in entries {
        hash_string(&mut hasher, entry.path.as_str());
        hash_u64(&mut hasher, entry.byte_length);
        hasher.update(entry.digest.as_bytes());
        hash_kind(&mut hasher, entry.kind);
    }
    Digest::new(hasher.finalize().into())
}

fn hash_kind(hasher: &mut Sha256, kind: InputFileKind) {
    let bytes = match kind {
        InputFileKind::Audio(format) => [0, audio_format_tag(format)],
        InputFileKind::CueSheet => [1, 0],
        InputFileKind::Booklet => [2, 0],
        InputFileKind::CoverCandidate => [3, 0],
        InputFileKind::Other => [4, 0],
    };
    hasher.update(bytes);
}

fn audio_format_tag(format: AudioFormat) -> u8 {
    match format {
        AudioFormat::Wav => 0,
        AudioFormat::Flac => 1,
        AudioFormat::Alac => 2,
        AudioFormat::Aac => 3,
        AudioFormat::Mp3 => 4,
        AudioFormat::Other => 5,
    }
}

pub(crate) fn hash_string(hasher: &mut Sha256, value: &str) {
    hash_u64(hasher, value.len() as u64);
    hasher.update(value.as_bytes());
}

pub(crate) fn hash_u64(hasher: &mut Sha256, value: u64) {
    hasher.update(value.to_be_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, length: u64) -> ManifestEntry {
        ManifestEntry::new(
            SafeRelativePath::new(path).unwrap(),
            length,
            Digest::new([length as u8; Digest::LENGTH]),
            InputFileKind::Audio(AudioFormat::Wav),
        )
    }

    #[test]
    fn manifest_identity_is_order_independent_but_content_sensitive() {
        let forward =
            InputManifest::new(vec![entry("disc.wav", 10), entry("disc.cue", 20)]).unwrap();
        let reverse =
            InputManifest::new(vec![entry("disc.cue", 20), entry("disc.wav", 10)]).unwrap();
        let changed =
            InputManifest::new(vec![entry("disc.wav", 11), entry("disc.cue", 20)]).unwrap();

        assert_eq!(forward.digest(), reverse.digest());
        assert_ne!(forward.digest(), changed.digest());
        assert_eq!(forward.entries()[0].path().as_str(), "disc.cue");
    }
}
