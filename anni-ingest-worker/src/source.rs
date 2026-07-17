use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

use anni_ingest::{Digest, InputFileKind, InputManifest, ManifestEntry, SafeRelativePath};
use sha2::{Digest as ShaDigest, Sha256};

use crate::{join_protocol_path, WorkerError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSpec {
    path: SafeRelativePath,
    kind: InputFileKind,
}

impl SourceSpec {
    pub const fn new(path: SafeRelativePath, kind: InputFileKind) -> Self {
        Self { path, kind }
    }
}

#[derive(Debug, Clone)]
pub struct SourceTree {
    root: PathBuf,
}

impl SourceTree {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, WorkerError> {
        let root = fs::canonicalize(root)?;
        if !root.is_dir() {
            return Err(WorkerError::SourceRootNotDirectory { path: root });
        }
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn inspect(&self, specs: Vec<SourceSpec>) -> Result<InputManifest, WorkerError> {
        let mut entries = Vec::with_capacity(specs.len());
        for spec in specs {
            let mut file = self.open_file(&spec.path)?;
            let (digest, byte_length) = digest_reader(&mut file)?;
            entries.push(ManifestEntry::new(
                spec.path,
                byte_length,
                digest,
                spec.kind,
            ));
        }
        Ok(InputManifest::new(entries)?)
    }

    pub(crate) fn verify_manifest(&self, manifest: &InputManifest) -> Result<(), WorkerError> {
        for entry in manifest.entries() {
            self.open_verified(entry)?;
        }
        Ok(())
    }

    pub(crate) fn open_verified(&self, entry: &ManifestEntry) -> Result<File, WorkerError> {
        let mut file = self.open_file(entry.path())?;
        let (actual_digest, actual_length) = digest_reader(&mut file)?;
        if actual_length != entry.byte_length() {
            return Err(WorkerError::SourceLengthChanged {
                path: entry.path().clone(),
                expected: entry.byte_length(),
                actual: actual_length,
            });
        }
        if actual_digest != entry.digest() {
            return Err(WorkerError::SourceDigestChanged {
                path: entry.path().clone(),
                expected: entry.digest(),
                actual: actual_digest,
            });
        }
        file.seek(SeekFrom::Start(0))?;
        Ok(file)
    }

    fn open_file(&self, path: &SafeRelativePath) -> Result<File, WorkerError> {
        let joined = join_protocol_path(&self.root, path);
        let resolved = fs::canonicalize(joined)?;
        if !resolved.starts_with(&self.root) {
            return Err(WorkerError::SourceEscapesRoot {
                path: path.clone(),
                root: self.root.clone(),
            });
        }
        if !resolved.is_file() {
            return Err(WorkerError::SourceNotFile { path: path.clone() });
        }
        Ok(File::open(resolved)?)
    }
}

pub(crate) fn digest_reader(reader: &mut impl Read) -> Result<(Digest, u64), WorkerError> {
    let mut hasher = Sha256::new();
    let mut byte_length = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        byte_length += read as u64;
    }
    Ok((Digest::new(hasher.finalize().into()), byte_length))
}
