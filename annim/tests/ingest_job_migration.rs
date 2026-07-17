#![cfg(feature = "sqlite")]

use anni_ingest::{Digest, JobState};
use annim::migrator::Migrator;
use sea_orm::{prelude::Uuid, ConnectOptions, ConnectionTrait, Database, DbBackend, Statement};
use sea_orm_migration::{MigratorTrait, SchemaManager};

async fn migrated_database() -> sea_orm::DatabaseConnection {
    // A SQLite in-memory database belongs to one connection. Limiting the pool
    // keeps the migration and assertions on the same database instance.
    let mut options = ConnectOptions::new("sqlite::memory:");
    options.max_connections(1);
    let database = Database::connect(options).await.unwrap();
    Migrator::up(&database, None).await.unwrap();
    database
}

async fn insert_job(
    database: &sea_orm::DatabaseConnection,
    job_id: Uuid,
) -> Result<(), sea_orm::DbErr> {
    database
        .execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO ingest_job (job_id) VALUES (?)",
            [job_id.into()],
        ))
        .await
        .map(|_| ())
}

#[tokio::test]
async fn migration_creates_job_with_domain_defaults() {
    let database = migrated_database().await;
    let manager = SchemaManager::new(&database);
    assert!(manager.has_table("ingest_job").await.unwrap());

    let job_id = Uuid::new_v4();
    insert_job(&database, job_id).await.unwrap();

    let row = database
        .query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT state, metadata_revision, row_version FROM ingest_job WHERE job_id = ?",
            [job_id.into()],
        ))
        .await
        .unwrap()
        .unwrap();

    assert_eq!(
        row.try_get::<String>("", "state").unwrap(),
        JobState::Created.as_str()
    );
    assert_eq!(row.try_get::<i64>("", "metadata_revision").unwrap(), 1);
    assert_eq!(row.try_get::<i64>("", "row_version").unwrap(), 1);
}

#[tokio::test]
async fn migration_enforces_stable_job_identity() {
    let database = migrated_database().await;
    let job_id = Uuid::new_v4();

    insert_job(&database, job_id).await.unwrap();
    let duplicate = insert_job(&database, job_id).await;

    assert!(duplicate.is_err());
}

#[tokio::test]
async fn migration_enforces_one_metadata_document_per_job_revision() {
    let database = migrated_database().await;
    let manager = SchemaManager::new(&database);
    assert!(manager.has_table("ingest_metadata_revision").await.unwrap());

    let job_id = Uuid::new_v4();
    insert_job(&database, job_id).await.unwrap();
    let insert = || {
        database.execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO ingest_metadata_revision \
             (job_id, revision, document, document_sha256) VALUES (?, ?, ?, ?)",
            [
                job_id.into(),
                1_i64.into(),
                "{}".into(),
                vec![0_u8; Digest::LENGTH].into(),
            ],
        ))
    };

    insert().await.unwrap();
    assert!(insert().await.is_err());
}
