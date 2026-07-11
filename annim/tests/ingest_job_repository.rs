#![cfg(feature = "sqlite")]

use anni_ingest::{
    AlbumField, Confidence, Digest, Evidence, EvidenceMethod, EvidenceSourceKind, FieldPath,
    IngestJob, JobState, MetadataCandidate, MetadataDraft, MetadataRevision, MetadataValue,
};
use annim::{
    ingest::{
        IngestCommand, IngestJobRepository, IngestRepositoryError, IngestService, RowVersion,
    },
    migrator::Migrator,
};
use sea_orm::{
    prelude::Uuid, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend,
    Statement,
};
use sea_orm_migration::MigratorTrait;

async fn migrated_database() -> DatabaseConnection {
    let mut options = ConnectOptions::new("sqlite::memory:");
    options.max_connections(1);
    let database = Database::connect(options).await.unwrap();
    Migrator::up(&database, None).await.unwrap();
    database
}

fn booklet_title(id: Uuid, value: &str) -> MetadataCandidate {
    MetadataCandidate::new(
        id,
        FieldPath::Album(AlbumField::Title),
        MetadataValue::Text(value.to_owned()),
        Evidence::new(
            EvidenceSourceKind::CdBooklet,
            "booklet.pdf#page=2",
            Some("front cover title".to_owned()),
            EvidenceMethod::ManualTranscription,
        ),
        Confidence::new(10_000).unwrap(),
    )
    .unwrap()
}

#[tokio::test]
async fn repository_round_trips_and_lists_versioned_jobs() {
    let database = migrated_database().await;
    let repository = IngestJobRepository::new(database);
    let job = IngestJob::new(Uuid::new_v4());

    let mut stored = repository.create(&job).await.unwrap();
    assert_eq!(stored.row_version(), RowVersion::INITIAL);
    assert_eq!(stored.job(), &job);
    assert_eq!(
        repository
            .get_metadata_draft(job.id(), MetadataRevision::INITIAL)
            .await
            .unwrap()
            .unwrap()
            .draft(),
        &MetadataDraft::new(MetadataRevision::INITIAL)
    );
    assert!(matches!(
        repository.create(&job).await,
        Err(IngestRepositoryError::AlreadyExists { .. })
    ));

    stored.job_mut().begin_review().unwrap();
    repository.save(&mut stored).await.unwrap();

    let mut unsafe_revision = stored.clone();
    unsafe_revision
        .job_mut()
        .revise_metadata(MetadataRevision::INITIAL)
        .unwrap();
    assert!(matches!(
        repository.save(&mut unsafe_revision).await,
        Err(IngestRepositoryError::MetadataRevisionRequiresDocument { .. })
    ));

    assert_eq!(stored.row_version(), RowVersion::new(2).unwrap());
    assert_eq!(
        repository.get(job.id()).await.unwrap(),
        Some(stored.clone())
    );
    assert_eq!(
        repository
            .list(Some(JobState::Reviewing), 10, 0)
            .await
            .unwrap(),
        vec![stored]
    );
    assert!(repository
        .list(Some(JobState::Created), 10, 0)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn metadata_documents_preserve_unicode_and_fork_history() {
    let database = migrated_database().await;
    let repository = IngestJobRepository::new(database.clone());
    let job = IngestJob::new(Uuid::new_v4());
    let mut stored = repository.create(&job).await.unwrap();
    stored.job_mut().begin_review().unwrap();
    repository.save(&mut stored).await.unwrap();

    let exact = "曲名（Booklet） / 曲名(Booklet)・A〜B～C";
    let candidate_id = Uuid::new_v4();
    let mut draft = repository
        .get_metadata_draft(job.id(), MetadataRevision::INITIAL)
        .await
        .unwrap()
        .unwrap()
        .into_draft();
    draft
        .add_candidate(booklet_title(candidate_id, exact))
        .unwrap();
    draft.accept(candidate_id).unwrap();
    let persisted = repository
        .save_with_metadata(&mut stored, &draft)
        .await
        .unwrap();
    assert_eq!(
        persisted
            .draft()
            .accepted_value(FieldPath::Album(AlbumField::Title)),
        Some(&MetadataValue::Text(exact.to_owned()))
    );

    let service = IngestService::new(repository.clone());
    let revised = service
        .execute(
            job.id(),
            stored.row_version(),
            IngestCommand::ReviseMetadata {
                expected_revision: MetadataRevision::INITIAL,
            },
        )
        .await
        .unwrap();
    let next_revision = revised.job().metadata_revision();

    let revisions = repository.list_metadata_revisions(job.id()).await.unwrap();
    assert_eq!(revisions.len(), 2);
    assert_eq!(revisions[0].draft().revision(), next_revision);
    assert_eq!(
        revisions[1]
            .draft()
            .accepted_value(FieldPath::Album(AlbumField::Title)),
        Some(&MetadataValue::Text(exact.to_owned()))
    );

    database
        .execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE ingest_metadata_revision SET document = document || ' ' \
             WHERE job_id = ? AND revision = ?",
            [job.id().into(), 1_i64.into()],
        ))
        .await
        .unwrap();
    assert!(matches!(
        repository
            .get_metadata_draft(job.id(), MetadataRevision::INITIAL)
            .await,
        Err(IngestRepositoryError::MetadataDigestMismatch { .. })
    ));
}

#[tokio::test]
async fn stale_metadata_writer_cannot_leave_a_document_change() {
    let database = migrated_database().await;
    let repository = IngestJobRepository::new(database);
    let job = IngestJob::new(Uuid::new_v4());
    repository.create(&job).await.unwrap();

    let mut winner_job = repository.get(job.id()).await.unwrap().unwrap();
    let mut stale_job = winner_job.clone();
    winner_job.job_mut().begin_review().unwrap();
    stale_job.job_mut().begin_review().unwrap();
    let base = repository
        .get_metadata_draft(job.id(), MetadataRevision::INITIAL)
        .await
        .unwrap()
        .unwrap()
        .into_draft();
    let mut winner_draft = base.clone();
    winner_draft
        .add_candidate(booklet_title(Uuid::new_v4(), "Winner"))
        .unwrap();
    let mut stale_draft = base;
    stale_draft
        .add_candidate(booklet_title(Uuid::new_v4(), "Stale"))
        .unwrap();

    repository
        .save_with_metadata(&mut winner_job, &winner_draft)
        .await
        .unwrap();
    assert!(matches!(
        repository
            .save_with_metadata(&mut stale_job, &stale_draft)
            .await,
        Err(IngestRepositoryError::ConcurrentModification { .. })
    ));

    let current = repository
        .get_metadata_draft(job.id(), MetadataRevision::INITIAL)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(current.draft().candidates().len(), 1);
    assert_eq!(
        current.draft().candidates()[0].value(),
        &MetadataValue::Text("Winner".to_owned())
    );
}

#[tokio::test]
async fn stale_writer_is_rejected_without_overwriting_the_winner() {
    let database = migrated_database().await;
    let repository = IngestJobRepository::new(database);
    let job = IngestJob::new(Uuid::new_v4());
    repository.create(&job).await.unwrap();

    let mut first = repository.get(job.id()).await.unwrap().unwrap();
    let mut stale = first.clone();
    first.job_mut().begin_review().unwrap();
    stale.job_mut().begin_review().unwrap();

    repository.save(&mut first).await.unwrap();
    let error = repository.save(&mut stale).await.unwrap_err();

    assert!(matches!(
        error,
        IngestRepositoryError::ConcurrentModification {
            expected: RowVersion::INITIAL,
            actual,
            ..
        } if actual == RowVersion::new(2).unwrap()
    ));
    assert_eq!(repository.get(job.id()).await.unwrap(), Some(first));
}

#[tokio::test]
async fn corrupt_digest_is_rejected_at_the_repository_boundary() {
    let database = migrated_database().await;
    let repository = IngestJobRepository::new(database.clone());
    let job_id = Uuid::new_v4();

    database
        .execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO ingest_job \
             (job_id, state, metadata_revision, approved_revision, manifest_digest, plan_digest) \
             VALUES (?, ?, ?, ?, ?, ?)",
            [
                job_id.into(),
                JobState::Planned.as_str().into(),
                1_i64.into(),
                1_i64.into(),
                vec![1_u8].into(),
                vec![2_u8; Digest::LENGTH].into(),
            ],
        ))
        .await
        .unwrap();

    assert!(matches!(
        repository.get(job_id).await,
        Err(IngestRepositoryError::InvalidDigestLength {
            field: "manifest_digest",
            actual: 1,
            ..
        })
    ));
}
