#![cfg(feature = "sqlite")]

use anni_catalog::{CoverCandidateState, CoverSourceKind};
use annim::migrator::Migrator;
use sea_orm::{
    prelude::Uuid, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend,
    Statement,
};
use sea_orm_migration::{MigratorTrait, SchemaManager};

async fn migrated_database() -> DatabaseConnection {
    let mut options = ConnectOptions::new("sqlite::memory:");
    options.max_connections(1);
    let database = Database::connect(options).await.unwrap();
    Migrator::up(&database, None).await.unwrap();
    database
}

#[tokio::test]
async fn cover_schema_deduplicates_verified_bytes_and_one_selection_per_scope() {
    let database = migrated_database().await;
    let manager = SchemaManager::new(&database);
    for table in ["cover_asset", "cover_candidate", "cover_selection"] {
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

    let release_id = Uuid::new_v4();
    database
        .execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO catalog_release (release_id, artist_db_id, title) VALUES (?, ?, ?)",
            [
                release_id.into(),
                artist_db_id.into(),
                "作品・A〜B～C".into(),
            ],
        ))
        .await
        .unwrap();
    let release_db_id = database
        .query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT id FROM catalog_release WHERE release_id = ?",
            [release_id.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<i32>("", "id")
        .unwrap();

    let content_sha256 = vec![0x2a; 32];
    let asset_id = Uuid::new_v4();
    let storage_key = format!("sha256/2a/2a/{}.jpg", "2a".repeat(32));
    database
        .execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO cover_asset \
             (asset_id, content_sha256, storage_key, media_type, width, height, \
              byte_length, fetched_at, verified_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            [
                asset_id.into(),
                content_sha256.clone().into(),
                storage_key.into(),
                "image/jpeg".into(),
                4_000_i32.into(),
                4_000_i32.into(),
                8_000_000_i64.into(),
            ],
        ))
        .await
        .unwrap();
    let duplicate_digest = database
        .execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO cover_asset \
             (asset_id, content_sha256, storage_key, media_type, width, height, \
              byte_length, fetched_at, verified_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            [
                Uuid::new_v4().into(),
                content_sha256.into(),
                "sha256/duplicate.jpg".into(),
                "image/jpeg".into(),
                4_000_i32.into(),
                4_000_i32.into(),
                8_000_000_i64.into(),
            ],
        ))
        .await;
    assert!(duplicate_digest.is_err());
    let asset_db_id = database
        .query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT id FROM cover_asset WHERE asset_id = ?",
            [asset_id.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<i32>("", "id")
        .unwrap();

    let submitted_url =
        "https://m.media-amazon.com/images/I/81abc._AC_SL1500_.jpg?X-Amz-Signature=abc%2F123";
    let canonical_url = "https://m.media-amazon.com/images/I/81abc.jpg?X-Amz-Signature=abc%2F123";
    let candidate_id = Uuid::new_v4();
    database
        .execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO cover_candidate \
             (candidate_id, release_db_id, source_kind, submitted_url, canonical_url, \
              effective_url, state, asset_db_id, fetched_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)",
            [
                candidate_id.into(),
                release_db_id.into(),
                CoverSourceKind::Amazon.as_str().into(),
                submitted_url.into(),
                canonical_url.into(),
                canonical_url.into(),
                CoverCandidateState::Verified.as_str().into(),
                asset_db_id.into(),
            ],
        ))
        .await
        .unwrap();
    let candidate = database
        .query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT id, submitted_url, canonical_url FROM cover_candidate \
             WHERE candidate_id = ?",
            [candidate_id.into()],
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        candidate.try_get::<String>("", "submitted_url").unwrap(),
        submitted_url
    );
    assert_eq!(
        candidate.try_get::<String>("", "canonical_url").unwrap(),
        canonical_url
    );
    let candidate_db_id = candidate.try_get::<i32>("", "id").unwrap();

    let insert_selection = |selection_id: Uuid| {
        database.execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO cover_selection \
             (selection_id, release_db_id, disc_number, candidate_db_id, asset_db_id) \
             VALUES (?, ?, 0, ?, ?)",
            [
                selection_id.into(),
                release_db_id.into(),
                candidate_db_id.into(),
                asset_db_id.into(),
            ],
        ))
    };
    insert_selection(Uuid::new_v4()).await.unwrap();
    assert!(insert_selection(Uuid::new_v4()).await.is_err());
}
