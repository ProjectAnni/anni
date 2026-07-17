#![cfg(feature = "sqlite")]

use anni_catalog::ReleaseKind;
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
async fn catalog_collection_schema_preserves_exact_labels_and_defaults() {
    let database = migrated_database().await;
    let manager = SchemaManager::new(&database);
    for table in ["catalog_artist", "catalog_release", "collection_copy"] {
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
    let exact_title = "作品・A〜B～C (初回)";
    database
        .execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO catalog_release (release_id, artist_db_id, title) VALUES (?, ?, ?)",
            [release_id.into(), artist_db_id.into(), exact_title.into()],
        ))
        .await
        .unwrap();
    let release = database
        .query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT id, title, kind, wanted, unavailable, row_version \
             FROM catalog_release WHERE release_id = ?",
            [release_id.into()],
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(release.try_get::<String>("", "title").unwrap(), exact_title);
    assert_eq!(
        release.try_get::<String>("", "kind").unwrap(),
        ReleaseKind::Album.as_str()
    );
    assert!(!release.try_get::<bool>("", "wanted").unwrap());
    assert!(!release.try_get::<bool>("", "unavailable").unwrap());
    assert_eq!(release.try_get::<i64>("", "row_version").unwrap(), 1);

    let copy_id = Uuid::new_v4();
    let release_db_id = release.try_get::<i32>("", "id").unwrap();
    let insert_copy = || {
        database.execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO collection_copy \
             (copy_id, release_db_id, source_kind, source_label, codec) \
             VALUES (?, ?, ?, ?, ?)",
            [
                copy_id.into(),
                release_db_id.into(),
                "angel_anime".into(),
                "天使动漫".into(),
                "flac".into(),
            ],
        ))
    };
    insert_copy().await.unwrap();
    assert!(insert_copy().await.is_err());
}
