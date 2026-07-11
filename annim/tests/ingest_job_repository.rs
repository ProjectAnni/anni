#![cfg(feature = "sqlite")]

use anni_ingest::{Digest, IngestJob, JobState};
use annim::{
    ingest::{IngestJobRepository, IngestRepositoryError, RowVersion},
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

#[tokio::test]
async fn repository_round_trips_and_lists_versioned_jobs() {
    let database = migrated_database().await;
    let repository = IngestJobRepository::new(database);
    let job = IngestJob::new(Uuid::new_v4());

    let mut stored = repository.create(&job).await.unwrap();
    assert_eq!(stored.row_version(), RowVersion::INITIAL);
    assert_eq!(stored.job(), &job);
    assert!(matches!(
        repository.create(&job).await,
        Err(IngestRepositoryError::AlreadyExists { .. })
    ));

    stored.job_mut().begin_review().unwrap();
    repository.save(&mut stored).await.unwrap();

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
