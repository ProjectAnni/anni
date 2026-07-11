use std::{
    fs::{self, File},
    io::{Seek, SeekFrom},
    path::{Path, PathBuf},
};

use anni_ingest::{Digest, SafeRelativePath};

use crate::{join_protocol_path, source::digest_reader, WorkerError};

/// A local, trusted repository of assets that were fetched and verified
/// before an ingest plan was frozen.
///
/// This type intentionally exposes no network API. Every lookup is a safe
/// relative path resolved beneath one canonical repository root.
#[derive(Debug, Clone)]
pub struct AssetRepository {
    root: PathBuf,
}

impl AssetRepository {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, WorkerError> {
        let root = fs::canonicalize(root)?;
        if !root.is_dir() {
            return Err(WorkerError::AssetRepositoryRootNotDirectory { path: root });
        }
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn open_verified(
        &self,
        path: &SafeRelativePath,
        expected_digest: Digest,
        expected_length: u64,
    ) -> Result<File, WorkerError> {
        let mut file = self.open_file(path)?;
        let (actual_digest, actual_length) = digest_reader(&mut file)?;
        if actual_length != expected_length {
            return Err(WorkerError::AssetLengthMismatch {
                path: path.clone(),
                expected: expected_length,
                actual: actual_length,
            });
        }
        if actual_digest != expected_digest {
            return Err(WorkerError::AssetDigestMismatch {
                path: path.clone(),
                expected: expected_digest,
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
            return Err(WorkerError::AssetEscapesRepository {
                path: path.clone(),
                root: self.root.clone(),
            });
        }
        if !resolved.is_file() {
            return Err(WorkerError::AssetNotFile { path: path.clone() });
        }
        Ok(File::open(resolved)?)
    }
}
