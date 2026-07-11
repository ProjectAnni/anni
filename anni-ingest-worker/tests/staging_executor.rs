use std::{fs, io::Read};

use anni_common::traits::{Decode, Encode};
use anni_ingest::{
    AudioFormat, Digest, ExecutionPlan, InputFileKind, MetadataRevision, PlanOperation,
    SafeRelativePath, SplitOutputFormat,
};
use anni_ingest_worker::{SourceSpec, SourceTree, StagingExecutor, WorkerError};
use anni_split::codec::wav::WaveHeader;
use tempfile::tempdir;
use uuid::Uuid;

fn path(value: &str) -> SafeRelativePath {
    SafeRelativePath::new(value).unwrap()
}

fn write_test_wave(path: &std::path::Path) -> Vec<u8> {
    let header = WaveHeader {
        channels: 1,
        sample_rate: 150,
        byte_rate: 300,
        block_align: 2,
        bit_per_sample: 16,
        data_size: 600,
    };
    let mut bytes = Vec::new();
    header.write_to(&mut bytes).unwrap();
    bytes.extend((0..600).map(|index| (index % 251) as u8));
    fs::write(path, &bytes).unwrap();
    bytes
}

#[test]
fn executor_splits_and_copies_into_verified_staging_without_touching_sources() {
    let root = tempdir().unwrap();
    let source_root = root.path().join("source");
    let staging_parent = root.path().join("staging");
    fs::create_dir(&source_root).unwrap();
    fs::create_dir(&staging_parent).unwrap();

    let original_wave = write_test_wave(&source_root.join("album.wav"));
    let cue = r#"FILE "album.wav" WAVE
  TRACK 01 AUDIO
    TITLE "第一曲（原文）"
    INDEX 01 00:00:00
  TRACK 02 AUDIO
    TITLE "Second"
    INDEX 01 00:01:00
"#;
    fs::write(source_root.join("album.cue"), cue).unwrap();
    fs::write(source_root.join("cover.jpg"), b"cover bytes").unwrap();

    let source = SourceTree::open(&source_root).unwrap();
    let manifest = source
        .inspect(vec![
            SourceSpec::new(path("album.wav"), InputFileKind::Audio(AudioFormat::Wav)),
            SourceSpec::new(path("album.cue"), InputFileKind::CueSheet),
            SourceSpec::new(path("cover.jpg"), InputFileKind::CoverCandidate),
        ])
        .unwrap();
    let job_id = Uuid::new_v4();
    let plan = ExecutionPlan::new(
        job_id,
        MetadataRevision::INITIAL,
        &manifest,
        vec![
            PlanOperation::SplitCueWave {
                cue: path("album.cue"),
                wave: path("album.wav"),
                outputs: vec![path("tracks/01.wav"), path("tracks/02.wav")],
                format: SplitOutputFormat::Wav,
            },
            PlanOperation::CopyFile {
                source: path("cover.jpg"),
                target: path("cover.jpg"),
            },
        ],
    )
    .unwrap();

    let staging_root = staging_parent.join(job_id.to_string());
    let executor = StagingExecutor::create(source, &staging_root).unwrap();
    let receipt = executor.execute(&plan, &manifest).unwrap();

    assert_eq!(receipt.job_id(), job_id);
    assert_eq!(receipt.plan_digest(), plan.digest());
    assert_eq!(receipt.outputs().len(), 3);
    assert_ne!(receipt.digest(), Digest::new([0; Digest::LENGTH]));
    assert_eq!(
        fs::read(source_root.join("album.wav")).unwrap(),
        original_wave
    );
    assert_eq!(
        fs::read(source_root.join("album.cue")).unwrap(),
        cue.as_bytes()
    );
    assert_eq!(
        fs::read(source_root.join("cover.jpg")).unwrap(),
        b"cover bytes"
    );
    assert_eq!(
        fs::read(staging_root.join("cover.jpg")).unwrap(),
        b"cover bytes"
    );

    for output in ["tracks/01.wav", "tracks/02.wav"] {
        let mut file = fs::File::open(staging_root.join(output)).unwrap();
        let header = WaveHeader::from_reader(&mut file).unwrap();
        assert_eq!(header.data_size, 300);
        let mut body = Vec::new();
        file.read_to_end(&mut body).unwrap();
        assert_eq!(body.len(), 300);
    }
}

#[test]
fn executor_refuses_changed_sources_and_leaves_them_in_place() {
    let root = tempdir().unwrap();
    let source_root = root.path().join("source");
    let staging_parent = root.path().join("staging");
    fs::create_dir(&source_root).unwrap();
    fs::create_dir(&staging_parent).unwrap();
    fs::write(source_root.join("cover.jpg"), b"original").unwrap();

    let source = SourceTree::open(&source_root).unwrap();
    let manifest = source
        .inspect(vec![SourceSpec::new(
            path("cover.jpg"),
            InputFileKind::CoverCandidate,
        )])
        .unwrap();
    let plan = ExecutionPlan::new(
        Uuid::new_v4(),
        MetadataRevision::INITIAL,
        &manifest,
        vec![PlanOperation::CopyFile {
            source: path("cover.jpg"),
            target: path("cover.jpg"),
        }],
    )
    .unwrap();
    fs::write(source_root.join("cover.jpg"), b"changed").unwrap();

    let staging_root = staging_parent.join("job");
    let executor = StagingExecutor::create(source, &staging_root).unwrap();
    assert!(matches!(
        executor.execute(&plan, &manifest),
        Err(WorkerError::SourceDigestChanged { .. }) | Err(WorkerError::SourceLengthChanged { .. })
    ));
    assert_eq!(fs::read(source_root.join("cover.jpg")).unwrap(), b"changed");
    assert!(!staging_root.join("cover.jpg").exists());
}

#[test]
fn executor_encodes_and_verifies_flac_when_codec_is_available() {
    if which::which("flac").is_err() {
        eprintln!("skipping FLAC integration path because the codec is unavailable");
        return;
    }

    let root = tempdir().unwrap();
    let source_root = root.path().join("source");
    let staging_parent = root.path().join("staging");
    fs::create_dir(&source_root).unwrap();
    fs::create_dir(&staging_parent).unwrap();
    let original_wave = write_test_wave(&source_root.join("album.wav"));
    let cue = r#"FILE "album.wav" WAVE
  TRACK 01 AUDIO
    INDEX 01 00:00:00
  TRACK 02 AUDIO
    INDEX 01 00:01:00
"#;
    fs::write(source_root.join("album.cue"), cue).unwrap();

    let source = SourceTree::open(&source_root).unwrap();
    let manifest = source
        .inspect(vec![
            SourceSpec::new(path("album.wav"), InputFileKind::Audio(AudioFormat::Wav)),
            SourceSpec::new(path("album.cue"), InputFileKind::CueSheet),
        ])
        .unwrap();
    let plan = ExecutionPlan::new(
        Uuid::new_v4(),
        MetadataRevision::INITIAL,
        &manifest,
        vec![PlanOperation::SplitCueWave {
            cue: path("album.cue"),
            wave: path("album.wav"),
            outputs: vec![path("01.flac"), path("02.flac")],
            format: SplitOutputFormat::Flac,
        }],
    )
    .unwrap();

    let staging_root = staging_parent.join("flac-job");
    let receipt = StagingExecutor::create(source, &staging_root)
        .unwrap()
        .execute(&plan, &manifest)
        .unwrap();

    assert_eq!(receipt.outputs().len(), 2);
    assert_eq!(
        &fs::read(staging_root.join("01.flac")).unwrap()[..4],
        b"fLaC"
    );
    assert_eq!(
        &fs::read(staging_root.join("02.flac")).unwrap()[..4],
        b"fLaC"
    );
    assert_eq!(
        fs::read(source_root.join("album.wav")).unwrap(),
        original_wave
    );
}
