#![cfg(feature = "sqlite")]

use anni_catalog::{CatalogSourceKind, SyncRunStatus};
use annim::migrator::Migrator;
use sea_orm::{
    prelude::Uuid, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend,
    Statement,
};
use sea_orm_migration::{MigratorTrait, SchemaManager};
use sha2::{Digest as _, Sha256};

async fn migrated_database() -> DatabaseConnection {
    let mut options = ConnectOptions::new("sqlite::memory:");
    options.max_connections(1);
    let database = Database::connect(options).await.unwrap();
    Migrator::up(&database, None).await.unwrap();
    database
}

#[tokio::test]
async fn catalog_source_schema_keeps_immutable_observation_revisions() {
    let database = migrated_database().await;
    let manager = SchemaManager::new(&database);
    for table in [
        "catalog_source",
        "catalog_sync_run",
        "catalog_source_release",
        "catalog_source_release_revision",
    ] {
        assert!(manager.has_table(table).await.unwrap(), "missing {table}");
    }

    let artist_id = Uuid::new_v4();
    database
        .execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO catalog_artist (artist_id, display_name) VALUES (?, ?)",
            [artist_id.into(), "Artist（公式）".into()],
        ))
        .await
        .unwrap();
    let artist_db_id = database
        .query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT id FROM catalog_artist WHERE artist_id = ?",
            [artist_id.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<i32>("", "id")
        .unwrap();

    let source_id = Uuid::new_v4();
    let insert_source = || {
        database.execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO catalog_source (source_id, artist_db_id, kind, locator) \
             VALUES (?, ?, ?, ?)",
            [
                source_id.into(),
                artist_db_id.into(),
                CatalogSourceKind::Vgmdb.as_str().into(),
                "https://vgmdb.net/artist/1234".into(),
            ],
        ))
    };
    insert_source().await.unwrap();
    assert!(insert_source().await.is_err());
    let source_db_id = database
        .query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT id FROM catalog_source WHERE source_id = ?",
            [source_id.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<i32>("", "id")
        .unwrap();

    let run_id = Uuid::new_v4();
    database
        .execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO catalog_sync_run (run_id, source_db_id) VALUES (?, ?)",
            [run_id.into(), source_db_id.into()],
        ))
        .await
        .unwrap();
    let status = database
        .query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT status FROM catalog_sync_run WHERE run_id = ?",
            [run_id.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<String>("", "status")
        .unwrap();
    assert_eq!(status, SyncRunStatus::Queued.as_str());

    let source_release_id = Uuid::new_v4();
    let insert_release = || {
        database.execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO catalog_source_release \
             (source_release_id, source_db_id, external_release_id, source_url) \
             VALUES (?, ?, ?, ?)",
            [
                source_release_id.into(),
                source_db_id.into(),
                "album/作品・A〜B～C".into(),
                "https://vgmdb.net/album/5678".into(),
            ],
        ))
    };
    insert_release().await.unwrap();
    assert!(insert_release().await.is_err());
    let source_release_db_id = database
        .query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT id FROM catalog_source_release WHERE source_release_id = ?",
            [source_release_id.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<i32>("", "id")
        .unwrap();

    let raw_document = r#"{"title":"作品・A〜B～C (初回)"}"#;
    let parsed_document = r#"{"title":"作品・A〜B～C (初回)","kind":"album"}"#;
    let raw_sha256 = Sha256::digest(raw_document.as_bytes()).to_vec();
    database
        .execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO catalog_source_release_revision \
             (source_release_db_id, revision, sync_run_id, raw_document, \
              parsed_document, raw_sha256) VALUES (?, ?, ?, ?, ?, ?)",
            [
                source_release_db_id.into(),
                1_i64.into(),
                run_id.into(),
                raw_document.into(),
                parsed_document.into(),
                raw_sha256.clone().into(),
            ],
        ))
        .await
        .unwrap();
    let revision = database
        .query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT raw_document, parsed_document, raw_sha256 \
             FROM catalog_source_release_revision \
             WHERE source_release_db_id = ? AND revision = 1",
            [source_release_db_id.into()],
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        revision.try_get::<String>("", "raw_document").unwrap(),
        raw_document
    );
    assert_eq!(
        revision.try_get::<String>("", "parsed_document").unwrap(),
        parsed_document
    );
    assert_eq!(
        revision.try_get::<Vec<u8>>("", "raw_sha256").unwrap(),
        raw_sha256
    );
}
