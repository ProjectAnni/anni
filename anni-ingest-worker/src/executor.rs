use std::{
    fs::{self, File, OpenOptions},
    io::{Cursor, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::Command,
};

use anni_common::{
    decode::raw_to_string,
    traits::{Decode, Encode},
};
use anni_ingest::{
    ExecutionPlan, InputManifest, PlanOperation, SafeRelativePath, SplitOutputFormat,
};
use anni_split::{
    codec::{wav::WaveHeader, Encoder, FlacCommandEncoder},
    CueSplitPlan, TrackRange,
};

use crate::{
    join_protocol_path, source::digest_reader, AssetRepository, ExecutionReceipt, OutputReceipt,
    SourceTree, WorkerError,
};

const PARTIAL_DIRECTORY: &str = ".anni-partials";

pub struct StagingExecutor {
    source: SourceTree,
    asset_repository: Option<AssetRepository>,
    staging_root: PathBuf,
    partial_root: PathBuf,
}

impl StagingExecutor {
    pub fn create(source: SourceTree, staging_root: impl AsRef<Path>) -> Result<Self, WorkerError> {
        Self::create_inner(source, None, staging_root.as_ref())
    }

    /// Create an executor that can materialize already-verified assets from a
    /// local repository. The repository is read-only from the worker's point
    /// of view and may not contain the staging directory.
    pub fn create_with_asset_repository(
        source: SourceTree,
        asset_repository: AssetRepository,
        staging_root: impl AsRef<Path>,
    ) -> Result<Self, WorkerError> {
        Self::create_inner(source, Some(asset_repository), staging_root.as_ref())
    }

    fn create_inner(
        source: SourceTree,
        asset_repository: Option<AssetRepository>,
        requested: &Path,
    ) -> Result<Self, WorkerError> {
        if requested.exists() {
            return Err(WorkerError::StagingAlreadyExists {
                path: requested.to_owned(),
            });
        }

        let file_name = requested.file_name().ok_or_else(|| {
            WorkerError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "staging path has no final component",
            ))
        })?;
        let parent = requested
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let parent = fs::canonicalize(parent)?;
        let staging_root = parent.join(file_name);
        if staging_root.starts_with(source.root()) {
            return Err(WorkerError::StagingInsideSource { path: staging_root });
        }
        if asset_repository
            .as_ref()
            .is_some_and(|repository| staging_root.starts_with(repository.root()))
        {
            return Err(WorkerError::StagingInsideAssetRepository { path: staging_root });
        }

        fs::create_dir(&staging_root)?;
        let staging_root = fs::canonicalize(staging_root)?;
        let partial_root = staging_root.join(PARTIAL_DIRECTORY);
        fs::create_dir(&partial_root)?;

        Ok(Self {
            source,
            asset_repository,
            staging_root,
            partial_root,
        })
    }

    pub fn staging_root(&self) -> &Path {
        &self.staging_root
    }

    pub fn execute(
        &self,
        plan: &ExecutionPlan,
        manifest: &InputManifest,
    ) -> Result<ExecutionReceipt, WorkerError> {
        if plan.manifest_digest() != manifest.digest() {
            return Err(WorkerError::ManifestMismatch {
                plan: plan.manifest_digest(),
                actual: manifest.digest(),
            });
        }
        self.source.verify_manifest(manifest)?;

        let mut receipts = Vec::new();
        let mut partial_index = 0_usize;
        for operation in plan.operations() {
            match operation {
                PlanOperation::CopyFile { source, target } => {
                    receipts.push(self.copy_file(manifest, source, target, partial_index)?);
                    partial_index += 1;
                }
                PlanOperation::MaterializeAsset {
                    repository_path,
                    digest,
                    byte_length,
                    target,
                } => {
                    receipts.push(self.materialize_asset(
                        repository_path,
                        *digest,
                        *byte_length,
                        target,
                        partial_index,
                    )?);
                    partial_index += 1;
                }
                PlanOperation::SplitCueWave {
                    cue,
                    wave,
                    outputs,
                    format,
                } => {
                    let (mut split_receipts, used) =
                        self.split_wave(manifest, cue, wave, outputs, *format, partial_index)?;
                    receipts.append(&mut split_receipts);
                    partial_index += used;
                }
            }
        }

        Ok(ExecutionReceipt::new(
            plan.job_id(),
            manifest.digest(),
            plan.digest(),
            receipts,
        ))
    }

    fn copy_file(
        &self,
        manifest: &InputManifest,
        source: &SafeRelativePath,
        target: &SafeRelativePath,
        partial_index: usize,
    ) -> Result<OutputReceipt, WorkerError> {
        let entry = manifest
            .entry(source)
            .expect("execution plan source was validated against this manifest");
        let mut input = self.source.open_verified(entry)?;
        let target_path = self.prepare_target(target)?;
        let partial_path = self.partial_path(partial_index);
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&partial_path)?;
        std::io::copy(&mut input, &mut output)?;
        output.flush()?;
        output.sync_all()?;

        let (digest, byte_length) = digest_file(&partial_path)?;
        if byte_length != entry.byte_length() {
            return Err(WorkerError::SourceLengthChanged {
                path: source.clone(),
                expected: entry.byte_length(),
                actual: byte_length,
            });
        }
        if digest != entry.digest() {
            return Err(WorkerError::SourceDigestChanged {
                path: source.clone(),
                expected: entry.digest(),
                actual: digest,
            });
        }

        self.promote_partial(&partial_path, &target_path, target)?;
        Ok(OutputReceipt::new(target.clone(), byte_length, digest))
    }

    fn materialize_asset(
        &self,
        repository_path: &SafeRelativePath,
        expected_digest: anni_ingest::Digest,
        expected_length: u64,
        target: &SafeRelativePath,
        partial_index: usize,
    ) -> Result<OutputReceipt, WorkerError> {
        let repository = self
            .asset_repository
            .as_ref()
            .ok_or(WorkerError::AssetRepositoryNotConfigured)?;
        let mut input =
            repository.open_verified(repository_path, expected_digest, expected_length)?;
        let target_path = self.prepare_target(target)?;
        let partial_path = self.partial_path(partial_index);
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&partial_path)?;
        std::io::copy(&mut input, &mut output)?;
        output.flush()?;
        output.sync_all()?;

        // Verify the bytes in staging as well as the repository source. This
        // catches repository changes that race with the copy and makes the
        // receipt attest to the exact immutable plan input.
        let (actual_digest, actual_length) = digest_file(&partial_path)?;
        if actual_length != expected_length {
            return Err(WorkerError::AssetLengthMismatch {
                path: repository_path.clone(),
                expected: expected_length,
                actual: actual_length,
            });
        }
        if actual_digest != expected_digest {
            return Err(WorkerError::AssetDigestMismatch {
                path: repository_path.clone(),
                expected: expected_digest,
                actual: actual_digest,
            });
        }

        self.promote_partial(&partial_path, &target_path, target)?;
        Ok(OutputReceipt::new(
            target.clone(),
            actual_length,
            actual_digest,
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn split_wave(
        &self,
        manifest: &InputManifest,
        cue: &SafeRelativePath,
        wave: &SafeRelativePath,
        outputs: &[SafeRelativePath],
        format: SplitOutputFormat,
        first_partial_index: usize,
    ) -> Result<(Vec<OutputReceipt>, usize), WorkerError> {
        let cue_entry = manifest
            .entry(cue)
            .expect("execution plan CUE source was validated against this manifest");
        let wave_entry = manifest
            .entry(wave)
            .expect("execution plan WAV source was validated against this manifest");

        let mut cue_file = self.source.open_verified(cue_entry)?;
        let mut cue_bytes = Vec::new();
        cue_file.read_to_end(&mut cue_bytes)?;
        let cue_text = raw_to_string(&cue_bytes);

        let mut wave_file = self.source.open_verified(wave_entry)?;
        let header = WaveHeader::from_reader(&mut wave_file)?;
        let data_start = wave_file.stream_position()?;
        let split_plan = CueSplitPlan::new(&cue_text, &header)?;
        if split_plan.tracks().len() != outputs.len() {
            return Err(WorkerError::SplitOutputCountMismatch {
                expected: outputs.len(),
                actual: split_plan.tracks().len(),
            });
        }
        let planned_name = wave.as_str().rsplit('/').next().unwrap_or(wave.as_str());
        let cue_name = split_plan
            .source_file()
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(split_plan.source_file());
        if cue_name != planned_name {
            return Err(WorkerError::CueWaveMismatch {
                cue_file: split_plan.source_file().to_owned(),
                planned_wave: wave.clone(),
            });
        }

        let mut pending = Vec::with_capacity(outputs.len());
        for (index, (range, target)) in split_plan.tracks().iter().zip(outputs.iter()).enumerate() {
            let target_path = self.prepare_target(target)?;
            let partial_path = self.partial_path(first_partial_index + index);
            self.write_track(
                &mut wave_file,
                &header,
                data_start,
                range,
                format,
                &partial_path,
            )?;
            let (digest, byte_length) = digest_file(&partial_path)?;
            pending.push((
                target.clone(),
                target_path,
                partial_path,
                byte_length,
                digest,
            ));
        }

        wave_file.seek(SeekFrom::Start(0))?;
        let (actual_digest, actual_length) = digest_reader(&mut wave_file)?;
        if actual_length != wave_entry.byte_length() {
            return Err(WorkerError::SourceLengthChanged {
                path: wave.clone(),
                expected: wave_entry.byte_length(),
                actual: actual_length,
            });
        }
        if actual_digest != wave_entry.digest() {
            return Err(WorkerError::SourceDigestChanged {
                path: wave.clone(),
                expected: wave_entry.digest(),
                actual: actual_digest,
            });
        }

        let mut receipts = Vec::with_capacity(pending.len());
        for (target, target_path, partial_path, byte_length, digest) in pending {
            self.promote_partial(&partial_path, &target_path, &target)?;
            receipts.push(OutputReceipt::new(target, byte_length, digest));
        }
        Ok((receipts, outputs.len()))
    }

    fn write_track(
        &self,
        wave_file: &mut File,
        header: &WaveHeader,
        data_start: u64,
        range: &TrackRange,
        format: SplitOutputFormat,
        partial_path: &Path,
    ) -> Result<(), WorkerError> {
        wave_file.seek(SeekFrom::Start(data_start + u64::from(range.start())))?;
        let track_header = WaveHeader {
            data_size: range.byte_length(),
            ..header.clone()
        };

        match format {
            SplitOutputFormat::Wav => {
                let mut output = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(partial_path)?;
                track_header.write_to(&mut output)?;
                let actual = std::io::copy(
                    &mut wave_file.take(u64::from(range.byte_length())),
                    &mut output,
                )?;
                if actual != u64::from(range.byte_length()) {
                    return Err(WorkerError::ShortTrackRead {
                        track: range.track_number(),
                        expected: u64::from(range.byte_length()),
                        actual,
                    });
                }
                output.flush()?;
                output.sync_all()?;
            }
            SplitOutputFormat::Flac => {
                let mut header_bytes = Cursor::new([0_u8; 44]);
                track_header.write_to(&mut header_bytes)?;
                header_bytes.set_position(0);
                let body = wave_file.take(u64::from(range.byte_length()));
                FlacCommandEncoder(partial_path).encode(header_bytes.chain(body))?;
                verify_flac(partial_path)?;
            }
        }
        Ok(())
    }

    fn prepare_target(&self, target: &SafeRelativePath) -> Result<PathBuf, WorkerError> {
        let target_path = join_protocol_path(&self.staging_root, target);
        let parent = target_path
            .parent()
            .expect("safe relative path has a parent");
        fs::create_dir_all(parent)?;
        let resolved_parent = fs::canonicalize(parent)?;
        if !resolved_parent.starts_with(&self.staging_root) {
            return Err(WorkerError::TargetEscapesStaging {
                path: target.clone(),
            });
        }
        let target_path = resolved_parent.join(
            target_path
                .file_name()
                .expect("safe relative path has a final component"),
        );
        if target_path.exists() {
            return Err(WorkerError::TargetAlreadyExists {
                path: target.clone(),
            });
        }
        Ok(target_path)
    }

    fn partial_path(&self, index: usize) -> PathBuf {
        self.partial_root.join(format!("{index:08}.tmp"))
    }

    fn promote_partial(
        &self,
        partial: &Path,
        target_path: &Path,
        target: &SafeRelativePath,
    ) -> Result<(), WorkerError> {
        match fs::hard_link(partial, target_path) {
            Ok(()) => {
                fs::remove_file(partial)?;
                Ok(())
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                Err(WorkerError::TargetAlreadyExists {
                    path: target.clone(),
                })
            }
            Err(error) => Err(error.into()),
        }
    }
}

fn digest_file(path: &Path) -> Result<(anni_ingest::Digest, u64), WorkerError> {
    let mut file = File::open(path)?;
    digest_reader(&mut file)
}

fn verify_flac(path: &Path) -> Result<(), WorkerError> {
    let flac = which::which("flac").map_err(anni_split::error::SplitError::from)?;
    let output = Command::new(flac)
        .args(["--totally-silent", "--test"])
        .arg(path)
        .output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(WorkerError::FlacVerificationFailed {
            status: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        })
    }
}
