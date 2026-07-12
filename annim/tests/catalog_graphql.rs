#![cfg(feature = "sqlite")]

use annim::{auth::AuthConfig, graphql::build_schema, migrator::Migrator};
use async_graphql::Request;
use sea_orm::{prelude::Uuid, ConnectOptions, Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;

async fn test_schema() -> annim::graphql::MetadataSchema {
    let mut options = ConnectOptions::new("sqlite::memory:");
    options.max_connections(1);
    let database: DatabaseConnection = Database::connect(options).await.unwrap();
    Migrator::up(&database, None).await.unwrap();
    build_schema(database)
}

fn admin_request(query: impl Into<String>) -> Request {
    const TOKEN: &str = "0123456789abcdef0123456789abcdef";
    let admin = AuthConfig::new(TOKEN)
        .unwrap()
        .authenticate_bearer(&format!("Bearer {TOKEN}"))
        .unwrap();
    Request::new(query).data(admin)
}

fn error_code(response: &async_graphql::Response) -> Option<&async_graphql::Value> {
    response.errors.first()?.extensions.as_ref()?.get("code")
}

#[tokio::test]
async fn catalog_queries_require_an_admin_token() {
    let schema = test_schema().await;
    let response = schema.execute("{ catalogArtists { artistId } }").await;

    assert_eq!(response.errors.len(), 1);
    assert_eq!(response.errors[0].message, "Unauthorized");
}

#[tokio::test]
async fn graphql_drives_artist_collection_without_exposing_private_locators() {
    let schema = test_schema().await;
    let artist_id = Uuid::new_v4();
    let release_id = Uuid::new_v4();
    let copy_id = Uuid::new_v4();
    let manifest_digest = "5a".repeat(32);

    let artist = schema
        .execute(admin_request(format!(
            r#"mutation {{
                createCatalogArtist(input: {{
                    artistId: "{artist_id}"
                    displayName: "Artist（公式）"
                    sortName: "Artist"
                }}) {{ artistId displayName rowVersion }}
            }}"#
        )))
        .await;
    assert!(artist.errors.is_empty(), "{:?}", artist.errors);
    assert_eq!(
        artist.data.into_json().unwrap()["createCatalogArtist"]["displayName"],
        "Artist（公式）"
    );

    let release = schema
        .execute(admin_request(format!(
            r#"mutation {{
                createCatalogRelease(input: {{
                    releaseId: "{release_id}"
                    artistId: "{artist_id}"
                    title: "作品・A〜B～C (初回)"
                    edition: "初回限定盤（CD＋BD）"
                    catalog: "TEST-0001"
                    releaseDate: "2026-07"
                    kind: ALBUM
                }}) {{ releaseId title collectionState rowVersion }}
            }}"#
        )))
        .await;
    assert!(release.errors.is_empty(), "{:?}", release.errors);

    let wanted = schema
        .execute(admin_request(format!(
            r#"mutation {{
                executeCatalogReleaseCommand(input: {{
                    releaseId: "{release_id}"
                    expectedRowVersion: "1"
                    command: {{ markWanted: EXECUTE }}
                }}) {{ collectionState rowVersion }}
            }}"#
        )))
        .await;
    assert!(wanted.errors.is_empty(), "{:?}", wanted.errors);
    assert_eq!(
        wanted.data.into_json().unwrap()["executeCatalogReleaseCommand"]["collectionState"],
        "WANTED"
    );

    let recorded = schema
        .execute(admin_request(format!(
            r#"mutation {{
                executeCatalogReleaseCommand(input: {{
                    releaseId: "{release_id}"
                    expectedRowVersion: "2"
                    command: {{ recordCopy: {{
                        copyId: "{copy_id}"
                        sourceKind: ANGEL_ANIME
                        sourceLabel: "天使动漫（群友补档）"
                        privateLocator: "私有/不可公开/PT种子"
                        codec: FLAC
                        sampleRateHz: 96000
                        bitDepth: 24
                        channels: 2
                        trackCount: 12
                        byteLength: "1234567890"
                        manifestDigest: "{manifest_digest}"
                        qualityVerified: true
                        notes: "原始抓取・A〜B～C"
                    }} }}
                }}) {{
                    collectionState rowVersion
                    copies {{ sourceLabel qualityTier byteLength qualityVerified }}
                }}
            }}"#
        )))
        .await;
    assert!(recorded.errors.is_empty(), "{:?}", recorded.errors);
    let recorded = recorded.data.into_json().unwrap();
    assert_eq!(
        recorded["executeCatalogReleaseCommand"]["collectionState"],
        "ACQUIRED"
    );
    assert_eq!(recorded["executeCatalogReleaseCommand"]["rowVersion"], "3");
    assert_eq!(
        recorded["executeCatalogReleaseCommand"]["copies"][0]["qualityTier"],
        "HI_RES_LOSSLESS"
    );

    let collection = schema
        .execute(admin_request(format!(
            r#"{{
                catalogArtistCollection(artistId: "{artist_id}") {{
                    artist {{ displayName }}
                    summary {{ total acquired collected }}
                    releaseTotalCount
                    releases {{
                        title edition collectionState
                        copies {{ sourceKind sourceLabel codec sampleRateHz bitDepth }}
                    }}
                }}
            }}"#
        )))
        .await;
    assert!(collection.errors.is_empty(), "{:?}", collection.errors);
    let collection = collection.data.into_json().unwrap();
    let collection = &collection["catalogArtistCollection"];
    assert_eq!(collection["artist"]["displayName"], "Artist（公式）");
    assert_eq!(collection["summary"]["total"], "1");
    assert_eq!(collection["summary"]["acquired"], "1");
    assert_eq!(collection["summary"]["collected"], "1");
    assert_eq!(collection["releases"][0]["title"], "作品・A〜B～C (初回)");
    assert_eq!(
        collection["releases"][0]["copies"][0]["sourceLabel"],
        "天使动漫（群友补档）"
    );

    let private_query = schema
        .execute(admin_request(format!(
            r#"{{ catalogArtistCollection(artistId: "{artist_id}") {{
                releases {{ copies {{ privateLocator }} }}
            }} }}"#
        )))
        .await;
    assert!(!private_query.errors.is_empty());

    let stale = schema
        .execute(admin_request(format!(
            r#"mutation {{
                executeCatalogReleaseCommand(input: {{
                    releaseId: "{release_id}"
                    expectedRowVersion: "2"
                    command: {{ markUnavailable: EXECUTE }}
                }}) {{ rowVersion }}
            }}"#
        )))
        .await;
    assert_eq!(
        error_code(&stale),
        Some(&async_graphql::Value::from("CATALOG_RELEASE_CONFLICT"))
    );
}
