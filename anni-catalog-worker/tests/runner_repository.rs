#![cfg(feature = "sqlite")]

use anni_catalog::{CatalogSourceKind, SyncCoverage, SyncRunStatus};
use anni_catalog_worker::{
    AdapterFuture, AdapterObservation, AdapterPage, CatalogAdapter, CatalogAdapterRegistry,
    CatalogWorker, CatalogWorkerOutcome, CatalogWorkerPolicy,
};
use annim::{
    catalog::{
        CatalogRepository, CatalogService, CatalogSyncService, NewCatalogArtist, NewCatalogSource,
        NewCatalogSyncRun,
    },
    entities::catalog_source_release_revision,
    migrator::Migrator,
};
use sea_orm::{ConnectOptions, Database, EntityTrait};
use sea_orm_migration::MigratorTrait;

const RAW: &str = r#"{"title":"作品・A〜B～C（初回）"}"#;
const PARSED: &str = r#"{"schemaVersion":1,"title":"作品・A〜B～C（初回）","kind":"album"}"#;

struct FixtureAdapter;

impl CatalogAdapter for FixtureAdapter {
    fn source_kind(&self) -> CatalogSourceKind {
        CatalogSourceKind::Vgmdb
    }

    fn fetch_page<'a>(
        &'a self,
        _lease: &'a annim::catalog::CatalogSyncLease,
        cursor: Option<&'a str>,
    ) -> AdapterFuture<'a> {
        assert!(cursor.is_none());
        Box::pin(async {
            Ok(AdapterPage {
                observations: vec![AdapterObservation {
                    external_release_id: "album/100".to_owned(),
                    source_url: "https://vgmdb.net/album/100".to_owned(),
                    raw_document: RAW.to_owned(),
                    parsed_document: PARSED.to_owned(),
                }],
                next_cursor: None,
                checkpoint: None,
                coverage: SyncCoverage::FullSnapshot,
                complete: true,
                empty_full_snapshot_confirmed: false,
            })
        })
    }
}

#[tokio::test]
async fn runner_commits_exact_evidence_through_the_real_repository() {
    let mut options = ConnectOptions::new("sqlite::memory:");
    options.max_connections(1);
    let database = Database::connect(options).await.unwrap();
    Migrator::up(&database, None).await.unwrap();

    let catalog = CatalogService::new(CatalogRepository::new(database.clone()));
    let artist = catalog
        .create_artist(NewCatalogArtist {
            artist_id: None,
            display_name: "Artist（公式）".to_owned(),
            sort_name: None,
            notes: None,
        })
        .await
        .unwrap();
    let sync = CatalogSyncService::new(database.clone());
    let source = sync
        .create_source(NewCatalogSource {
            source_id: None,
            artist_id: artist.artist_id,
            kind: CatalogSourceKind::Vgmdb,
            locator: "https://vgmdb.net/artist/1234".to_owned(),
            storefront: None,
            locale: Some("ja-JP".to_owned()),
            configuration_document: None,
            secret_ref: None,
        })
        .await
        .unwrap();
    let queued = sync
        .start_run(NewCatalogSyncRun {
            run_id: None,
            source_id: source.source_id,
            requested_cursor: None,
        })
        .await
        .unwrap();

    let mut adapters = CatalogAdapterRegistry::new();
    adapters.register(FixtureAdapter).unwrap();
    let worker =
        CatalogWorker::new(sync.clone(), adapters, CatalogWorkerPolicy::default()).unwrap();
    let outcome = worker.run_once().await.unwrap();
    assert!(matches!(
        outcome,
        CatalogWorkerOutcome::Succeeded {
            run_id,
            coverage: SyncCoverage::FullSnapshot,
            processed_count: 1,
        } if run_id == queued.run_id
    ));

    let finished = sync.get_run(queued.run_id).await.unwrap().unwrap();
    assert_eq!(finished.status, SyncRunStatus::Succeeded);
    assert_eq!(finished.coverage, SyncCoverage::FullSnapshot);
    assert!(finished.started_from_root);
    assert!(finished.snapshot_complete);
    assert_eq!(finished.observed_count, 1);
    assert_eq!(finished.attempt_count, 1);

    let revision = catalog_source_release_revision::Entity::find()
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(revision.raw_document, RAW);
    assert_eq!(revision.parsed_document, PARSED);
}
