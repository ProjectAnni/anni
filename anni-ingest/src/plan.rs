use std::collections::BTreeSet;

use sha2::{Digest as ShaDigest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    manifest::{hash_string, hash_u64},
    AudioFormat, Digest, InputFileKind, InputManifest, MetadataRevision, SafeRelativePath,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SplitOutputFormat {
    Wav,
    Flac,
}

/// Side-effect operations allowed inside a job-specific staging directory.
/// There is intentionally no delete or source-move operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanOperation {
    CopyFile {
        source: SafeRelativePath,
        target: SafeRelativePath,
    },
    SplitCueWave {
        cue: SafeRelativePath,
        wave: SafeRelativePath,
        outputs: Vec<SafeRelativePath>,
        format: SplitOutputFormat,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionPlan {
    job_id: Uuid,
    metadata_revision: MetadataRevision,
    manifest_digest: Digest,
    operations: Vec<PlanOperation>,
    digest: Digest,
}

impl ExecutionPlan {
    pub fn new(
        job_id: Uuid,
        metadata_revision: MetadataRevision,
        manifest: &InputManifest,
        operations: Vec<PlanOperation>,
    ) -> Result<Self, PlanError> {
        if operations.is_empty() {
            return Err(PlanError::Empty);
        }

        let mut targets = BTreeSet::new();
        for operation in &operations {
            match operation {
                PlanOperation::CopyFile { source, target } => {
                    require_source(manifest, source, None)?;
                    insert_target(&mut targets, target)?;
                }
                PlanOperation::SplitCueWave {
                    cue, wave, outputs, ..
                } => {
                    require_source(manifest, cue, Some(InputFileKind::CueSheet))?;
                    require_source(manifest, wave, Some(InputFileKind::Audio(AudioFormat::Wav)))?;
                    if outputs.is_empty() {
                        return Err(PlanError::EmptySplitOutputs);
                    }
                    for output in outputs {
                        insert_target(&mut targets, output)?;
                    }
                }
            }
        }

        let manifest_digest = manifest.digest();
        let digest = digest_plan(job_id, metadata_revision, manifest_digest, &operations);
        Ok(Self {
            job_id,
            metadata_revision,
            manifest_digest,
            operations,
            digest,
        })
    }

    pub const fn job_id(&self) -> Uuid {
        self.job_id
    }

    pub const fn metadata_revision(&self) -> MetadataRevision {
        self.metadata_revision
    }

    pub const fn manifest_digest(&self) -> Digest {
        self.manifest_digest
    }

    pub fn operations(&self) -> &[PlanOperation] {
        &self.operations
    }

    pub const fn digest(&self) -> Digest {
        self.digest
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PlanError {
    #[error("execution plan cannot be empty")]
    Empty,
    #[error("execution plan references source not present in manifest: {path}")]
    UnknownSource { path: SafeRelativePath },
    #[error("source {path} has kind {actual:?}, expected {expected:?}")]
    SourceKindMismatch {
        path: SafeRelativePath,
        expected: InputFileKind,
        actual: InputFileKind,
    },
    #[error("CUE/WAV split operation must declare at least one output")]
    EmptySplitOutputs,
    #[error("multiple operations write the same staging target: {path}")]
    DuplicateTarget { path: SafeRelativePath },
}

fn require_source(
    manifest: &InputManifest,
    path: &SafeRelativePath,
    expected: Option<InputFileKind>,
) -> Result<(), PlanError> {
    let entry = manifest
        .entry(path)
        .ok_or_else(|| PlanError::UnknownSource { path: path.clone() })?;
    if let Some(expected) = expected
        && entry.kind() != expected
    {
        return Err(PlanError::SourceKindMismatch {
            path: path.clone(),
            expected,
            actual: entry.kind(),
        });
    }
    Ok(())
}

fn insert_target(
    targets: &mut BTreeSet<SafeRelativePath>,
    target: &SafeRelativePath,
) -> Result<(), PlanError> {
    if targets.insert(target.clone()) {
        Ok(())
    } else {
        Err(PlanError::DuplicateTarget {
            path: target.clone(),
        })
    }
}

fn digest_plan(
    job_id: Uuid,
    metadata_revision: MetadataRevision,
    manifest_digest: Digest,
    operations: &[PlanOperation],
) -> Digest {
    let mut hasher = Sha256::new();
    hasher.update(b"anni-ingest-execution-plan-v1\0");
    hasher.update(job_id.as_bytes());
    hash_u64(&mut hasher, metadata_revision.get());
    hasher.update(manifest_digest.as_bytes());
    hash_u64(&mut hasher, operations.len() as u64);

    for operation in operations {
        match operation {
            PlanOperation::CopyFile { source, target } => {
                hasher.update([0]);
                hash_string(&mut hasher, source.as_str());
                hash_string(&mut hasher, target.as_str());
            }
            PlanOperation::SplitCueWave {
                cue,
                wave,
                outputs,
                format,
            } => {
                hasher.update([1]);
                hash_string(&mut hasher, cue.as_str());
                hash_string(&mut hasher, wave.as_str());
                hasher.update([match format {
                    SplitOutputFormat::Wav => 0,
                    SplitOutputFormat::Flac => 1,
                }]);
                hash_u64(&mut hasher, outputs.len() as u64);
                for output in outputs {
                    hash_string(&mut hasher, output.as_str());
                }
            }
        }
    }

    Digest::new(hasher.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ManifestEntry, MetadataRevision};

    fn path(value: &str) -> SafeRelativePath {
        SafeRelativePath::new(value).unwrap()
    }

    fn manifest() -> InputManifest {
        InputManifest::new(vec![
            ManifestEntry::new(
                path("disc.wav"),
                100,
                Digest::new([1; Digest::LENGTH]),
                InputFileKind::Audio(AudioFormat::Wav),
            ),
            ManifestEntry::new(
                path("disc.cue"),
                20,
                Digest::new([2; Digest::LENGTH]),
                InputFileKind::CueSheet,
            ),
            ManifestEntry::new(
                path("cover.jpg"),
                30,
                Digest::new([3; Digest::LENGTH]),
                InputFileKind::CoverCandidate,
            ),
        ])
        .unwrap()
    }

    fn split_operation() -> PlanOperation {
        PlanOperation::SplitCueWave {
            cue: path("disc.cue"),
            wave: path("disc.wav"),
            outputs: vec![path("tracks/01.flac"), path("tracks/02.flac")],
            format: SplitOutputFormat::Flac,
        }
    }

    #[test]
    fn plan_digest_binds_job_revision_manifest_and_exact_outputs() {
        let job_id = Uuid::new_v4();
        let manifest = manifest();
        let plan = ExecutionPlan::new(
            job_id,
            MetadataRevision::INITIAL,
            &manifest,
            vec![split_operation()],
        )
        .unwrap();
        let same = ExecutionPlan::new(
            job_id,
            MetadataRevision::INITIAL,
            &manifest,
            vec![split_operation()],
        )
        .unwrap();
        let changed = ExecutionPlan::new(
            job_id,
            MetadataRevision::new(2).unwrap(),
            &manifest,
            vec![split_operation()],
        )
        .unwrap();

        assert_eq!(plan.digest(), same.digest());
        assert_ne!(plan.digest(), changed.digest());
        assert_eq!(plan.manifest_digest(), manifest.digest());
    }

    #[test]
    fn plan_rejects_unknown_sources_wrong_roles_and_target_collisions() {
        let manifest = manifest();
        let job_id = Uuid::new_v4();

        let unknown = PlanOperation::CopyFile {
            source: path("missing.jpg"),
            target: path("cover.jpg"),
        };
        assert!(matches!(
            ExecutionPlan::new(job_id, MetadataRevision::INITIAL, &manifest, vec![unknown]),
            Err(PlanError::UnknownSource { .. })
        ));

        let wrong_role = PlanOperation::SplitCueWave {
            cue: path("cover.jpg"),
            wave: path("disc.wav"),
            outputs: vec![path("01.flac")],
            format: SplitOutputFormat::Flac,
        };
        assert!(matches!(
            ExecutionPlan::new(
                job_id,
                MetadataRevision::INITIAL,
                &manifest,
                vec![wrong_role]
            ),
            Err(PlanError::SourceKindMismatch { .. })
        ));

        let collision = PlanOperation::CopyFile {
            source: path("cover.jpg"),
            target: path("tracks/01.flac"),
        };
        assert!(matches!(
            ExecutionPlan::new(
                job_id,
                MetadataRevision::INITIAL,
                &manifest,
                vec![split_operation(), collision]
            ),
            Err(PlanError::DuplicateTarget { .. })
        ));
    }
}
