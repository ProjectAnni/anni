#![cfg(feature = "sqlite")]

use std::{num::NonZeroU32, time::Duration};

use anni_catalog::{cover_asset_storage_key, CoverCandidateState, CoverMediaType, CoverSourceKind};
use anni_ingest::Digest;
use annim::{
    catalog::{CatalogRepository, CatalogService, NewCatalogArtist, NewCatalogRelease},
    cover::{
        CoverError, CoverRepository, CoverRowVersion, CoverService, NewCoverCandidate, SelectCover,
        VerifiedCoverAsset,
    },
    migrator::Migrator,
};
use sea_orm::{prelude::Uuid, ConnectOptions, Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;

async fn services() -> (CatalogService, CoverService) {
    let mut options = ConnectOptions::new("sqlite::memory:");
    options.max_connections(1);
    let database: DatabaseConnection = Database::connect(options).await.unwrap();
    Migrator::up(&database, None).await.unwrap();
    (
        CatalogService::new(CatalogRepository::new(database.clone())),
        CoverService::new(CoverRepository::new(database)),
    )
}

async fn release(catalog: &CatalogService) -> Uuid {
    let artist_id = Uuid::new_v4();
    catalog
        .create_artist(NewCatalogArtist {
            artist_id: Some(artist_id),
            display_name: "Artist（公式）".to_owned(),
            sort_name: None,
            notes: None,
        })
        .await
        .unwrap();
    let release_id = Uuid::new_v4();
    catalog
        .create_release(NewCatalogRelease {
            release_id: Some(release_id),
            artist_id,
            title: "作品・A〜B～C".to_owned(),
            edition: None,
            catalog: None,
            release_date: None,
            kind: anni_catalog::ReleaseKind::Album,
            notes: None,
        })
        .await
        .unwrap();
    release_id
}

fn verified_asset(effective_url: String) -> VerifiedCoverAsset {
    VerifiedCoverAsset {
        content_sha256: Digest::new([0xab; Digest::LENGTH]),
        media_type: CoverMediaType::Jpeg,
        width: NonZeroU32::new(4_000).unwrap(),
        height: NonZeroU32::new(4_000).unwrap(),
        byte_length: 8_000_000,
        effective_url: Some(effective_url),
    }
}

#[tokio::test]
async fn verified_candidate_becomes_a_content_frozen_selection() {
    let (catalog, covers) = services().await;
    let release_id = release(&catalog).await;
    let candidate = covers
        .create_candidate(NewCoverCandidate {
            candidate_id: None,
            release_id,
            disc_number: 0,
            source_kind: CoverSourceKind::Amazon,
            source_release_revision_db_id: None,
            submitted_url: "https://m.media-amazon.com/images/I/81abc._AC_SL1500_.jpg?X-Amz-Signature=abc%2F123".to_owned(),
        })
        .await
        .unwrap();
    assert_eq!(candidate.state, CoverCandidateState::Discovered);
    assert!(candidate.has_remote_url);

    let queued = covers
        .queue_candidate(candidate.candidate_id, candidate.row_version, None)
        .await
        .unwrap();
    assert_eq!(queued.state, CoverCandidateState::Queued);
    let lease = covers
        .claim_next(Duration::from_secs(60))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(lease.candidate_id, candidate.candidate_id);
    assert!(lease.request_url.contains("/81abc.jpg?"));
    assert!(lease.request_url.contains("X-Amz-Signature=abc%2F123"));

    let verified = covers
        .complete_fetch(
            lease.candidate_id,
            lease.row_version,
            lease.lease_token,
            verified_asset(lease.request_url),
        )
        .await
        .unwrap();
    assert_eq!(verified.state, CoverCandidateState::Verified);
    let asset = verified.asset.as_ref().unwrap();
    assert_eq!(
        asset.storage_key,
        cover_asset_storage_key(asset.content_sha256.as_bytes(), CoverMediaType::Jpeg)
    );

    let selection = covers
        .select_cover(SelectCover {
            release_id,
            disc_number: 0,
            candidate_id: verified.candidate_id,
            expected_row_version: None,
        })
        .await
        .unwrap();
    assert_eq!(selection.row_version, CoverRowVersion::INITIAL);
    assert_eq!(selection.asset.content_sha256, asset.content_sha256);
}

#[tokio::test]
async fn an_expired_worker_lease_cannot_publish_an_asset() {
    let (catalog, covers) = services().await;
    let release_id = release(&catalog).await;
    let candidate = covers
        .create_candidate(NewCoverCandidate {
            candidate_id: None,
            release_id,
            disc_number: 0,
            source_kind: CoverSourceKind::ArtistWebsite,
            source_release_revision_db_id: None,
            submitted_url: "https://artist.example/ジャケット（初回）.jpg".to_owned(),
        })
        .await
        .unwrap();
    covers
        .queue_candidate(candidate.candidate_id, candidate.row_version, None)
        .await
        .unwrap();
    let expired = covers
        .repository()
        .claim_next_at(
            chrono::Utc::now() - chrono::Duration::minutes(5),
            Duration::from_secs(1),
        )
        .await
        .unwrap()
        .unwrap();

    let stale = covers
        .complete_fetch(
            expired.candidate_id,
            expired.row_version,
            expired.lease_token,
            verified_asset(expired.request_url),
        )
        .await;
    assert!(matches!(stale, Err(CoverError::LeaseMismatch { .. })));
    let reclaimed = covers
        .claim_next(Duration::from_secs(60))
        .await
        .unwrap()
        .unwrap();
    assert_ne!(reclaimed.lease_token, expired.lease_token);
    assert!(reclaimed.row_version > expired.row_version);
}
