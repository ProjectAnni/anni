#![cfg(feature = "sqlite")]

use std::num::NonZeroU32;

use anni_catalog::{AcquisitionSourceKind, AudioCodec, CollectionState, QualityTier, ReleaseKind};
use anni_ingest::Digest;
use annim::{
    catalog::{
        CatalogError, CatalogReleaseCommand, CatalogRepository, CatalogRowVersion, CatalogService,
        NewCatalogArtist, NewCatalogRelease, NewCollectionCopy,
    },
    migrator::Migrator,
};
use sea_orm::{prelude::Uuid, ConnectOptions, Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;

async fn service() -> CatalogService {
    let mut options = ConnectOptions::new("sqlite::memory:");
    options.max_connections(1);
    let database: DatabaseConnection = Database::connect(options).await.unwrap();
    Migrator::up(&database, None).await.unwrap();
    CatalogService::new(CatalogRepository::new(database))
}

fn hi_res_copy(copy_id: Uuid) -> NewCollectionCopy {
    NewCollectionCopy {
        copy_id: Some(copy_id),
        source_kind: AcquisitionSourceKind::AngelAnime,
        source_label: "天使动漫（群友补档）".to_owned(),
        private_locator: Some("私有/不可公开/PT种子".to_owned()),
        codec: AudioCodec::Flac,
        sample_rate_hz: NonZeroU32::new(96_000),
        bit_depth: Some(24),
        channels: Some(2),
        track_count: Some(12),
        byte_length: Some(1_234_567_890),
        manifest_digest: Some(Digest::new([0x5a; Digest::LENGTH])),
        quality_verified: true,
        ingest_job_id: None,
        notes: Some("原始抓取・A〜B～C".to_owned()),
    }
}

async fn create_release(service: &CatalogService) -> (Uuid, Uuid) {
    let artist_id = Uuid::new_v4();
    service
        .create_artist(NewCatalogArtist {
            artist_id: Some(artist_id),
            display_name: "Artist（公式）".to_owned(),
            sort_name: Some("Artist".to_owned()),
            notes: None,
        })
        .await
        .unwrap();
    let release_id = Uuid::new_v4();
    service
        .create_release(NewCatalogRelease {
            release_id: Some(release_id),
            artist_id,
            title: "作品・A〜B～C (初回)".to_owned(),
            edition: Some("初回限定盤（CD＋BD）".to_owned()),
            catalog: Some("TEST-0001".to_owned()),
            release_date: Some("2026-07".to_owned()),
            kind: ReleaseKind::Album,
            notes: None,
        })
        .await
        .unwrap();
    (artist_id, release_id)
}

#[tokio::test]
async fn artist_collection_round_trips_exact_text_and_measured_quality() {
    let service = service().await;
    let (artist_id, release_id) = create_release(&service).await;
    let recorded = service
        .execute_release_command(
            release_id,
            CatalogRowVersion::INITIAL,
            CatalogReleaseCommand::RecordCopy(hi_res_copy(Uuid::new_v4())),
        )
        .await
        .unwrap();

    assert_eq!(recorded.title, "作品・A〜B～C (初回)");
    assert_eq!(recorded.collection_state(), CollectionState::Acquired);
    assert_eq!(recorded.row_version.get(), 2);
    assert_eq!(recorded.copies.len(), 1);
    assert_eq!(recorded.copies[0].source_label, "天使动漫（群友补档）");
    assert_eq!(
        recorded.copies[0].quality_tier(),
        QualityTier::HiResLossless
    );
    assert_eq!(
        recorded.copies[0].manifest_digest,
        Some(Digest::new([0x5a; Digest::LENGTH]))
    );

    let collection = service
        .artist_collection(artist_id, None)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(collection.artist.display_name, "Artist（公式）");
    assert_eq!(collection.summary.total, 1);
    assert_eq!(collection.summary.acquired, 1);
    assert_eq!(collection.summary.collected, 1);
}

#[tokio::test]
async fn copy_insert_failure_and_stale_writer_leave_the_aggregate_unchanged() {
    let service = service().await;
    let (_, release_id) = create_release(&service).await;
    let copy_id = Uuid::new_v4();
    let recorded = service
        .execute_release_command(
            release_id,
            CatalogRowVersion::INITIAL,
            CatalogReleaseCommand::RecordCopy(hi_res_copy(copy_id)),
        )
        .await
        .unwrap();
    assert_eq!(recorded.row_version.get(), 2);

    let duplicate = service
        .execute_release_command(
            release_id,
            recorded.row_version,
            CatalogReleaseCommand::RecordCopy(hi_res_copy(copy_id)),
        )
        .await;
    assert!(matches!(
        duplicate,
        Err(CatalogError::CopyAlreadyExists { copy_id: actual }) if actual == copy_id
    ));
    let after_duplicate = service
        .repository()
        .get_release(release_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after_duplicate.row_version.get(), 2);
    assert_eq!(after_duplicate.copies.len(), 1);

    let stale_copy_id = Uuid::new_v4();
    let stale = service
        .execute_release_command(
            release_id,
            CatalogRowVersion::INITIAL,
            CatalogReleaseCommand::RecordCopy(hi_res_copy(stale_copy_id)),
        )
        .await;
    assert!(matches!(stale, Err(CatalogError::ReleaseConflict { .. })));
    let after_stale = service
        .repository()
        .get_release(release_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after_stale.row_version.get(), 2);
    assert_eq!(after_stale.copies.len(), 1);
    assert!(after_stale
        .copies
        .iter()
        .all(|copy| copy.copy_id != stale_copy_id));
}
