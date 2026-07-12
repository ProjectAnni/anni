#![cfg(feature = "sqlite")]

use annim::{
    auth::AuthConfig,
    catalog::CatalogSyncService,
    config::{CatalogSyncProvisioningConfig, APPLE_MUSIC_SECRET_REF_ENV},
    graphql::{build_schema, schema_builder_with_catalog_sync_service},
    migrator::Migrator,
};
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

async fn configured_test_schema() -> annim::graphql::MetadataSchema {
    let mut options = ConnectOptions::new("sqlite::memory:");
    options.max_connections(1);
    let database: DatabaseConnection = Database::connect(options).await.unwrap();
    Migrator::up(&database, None).await.unwrap();
    let provisioning = CatalogSyncProvisioningConfig::from_lookup(|name| {
        (name == APPLE_MUSIC_SECRET_REF_ENV).then(|| "apple/developer-token.jwt".to_owned())
    })
    .unwrap();
    let service = CatalogSyncService::with_provisioning(database.clone(), provisioning);
    schema_builder_with_catalog_sync_service(database, service).finish()
}

fn admin_request(query: impl Into<String>) -> Request {
    const TOKEN: &str = "0123456789abcdef0123456789abcdef";
    let admin = AuthConfig::new(TOKEN)
        .unwrap()
        .authenticate_bearer(&format!("Bearer {TOKEN}"))
        .unwrap();
    Request::new(query).data(admin)
}

#[tokio::test]
async fn graphql_queues_catalog_sync_without_exposing_adapter_evidence_or_secrets() {
    let schema = configured_test_schema().await;
    let artist_id = Uuid::new_v4();
    let source_id = Uuid::new_v4();
    let run_id = Uuid::new_v4();

    let artist = schema
        .execute(admin_request(format!(
            r#"mutation {{
                createCatalogArtist(input: {{
                    artistId: "{artist_id}"
                    displayName: "Artist（公式）"
                }}) {{ artistId }}
            }}"#
        )))
        .await;
    assert!(artist.errors.is_empty(), "{:?}", artist.errors);

    let source = schema
        .execute(admin_request(format!(
            r#"mutation {{
                createCatalogSyncSource(input: {{
                    sourceId: "{source_id}"
                    artistId: "{artist_id}"
                    kind: APPLE_MUSIC
                    locator: "1370700"
                    storefront: "jp"
                    locale: "ja-JP"
                }}) {{
                    sourceId artistId kind storefront locale enabled provisioningState rowVersion
                }}
            }}"#
        )))
        .await;
    assert!(source.errors.is_empty(), "{:?}", source.errors);
    let source = source.data.into_json().unwrap();
    assert_eq!(
        source["createCatalogSyncSource"]["sourceId"],
        source_id.to_string()
    );
    assert_eq!(source["createCatalogSyncSource"]["kind"], "APPLE_MUSIC");
    assert_eq!(
        source["createCatalogSyncSource"]["provisioningState"],
        "READY_TO_QUEUE"
    );
    assert_eq!(source["createCatalogSyncSource"]["rowVersion"], "1");

    let sources = schema
        .execute(admin_request(format!(
            r#"{{ catalogSyncSources(artistId: "{artist_id}") {{
                sourceId kind storefront locale enabled provisioningState rowVersion
            }} }}"#
        )))
        .await;
    assert!(sources.errors.is_empty(), "{:?}", sources.errors);
    let sources = sources.data.into_json().unwrap();
    assert_eq!(sources["catalogSyncSources"].as_array().unwrap().len(), 1);
    assert_eq!(
        sources["catalogSyncSources"][0]["sourceId"],
        source_id.to_string()
    );
    assert_eq!(
        sources["catalogSyncSources"][0]["provisioningState"],
        "READY_TO_QUEUE"
    );

    let run = schema
        .execute(admin_request(format!(
            r#"mutation {{
                startCatalogSyncRun(input: {{ runId: "{run_id}", sourceId: "{source_id}" }}) {{
                    runId sourceId status coverage startedFromRoot snapshotComplete
                    observedCount attemptCount rowVersion startedAt finishedAt
                }}
            }}"#
        )))
        .await;
    assert!(run.errors.is_empty(), "{:?}", run.errors);
    let run = run.data.into_json().unwrap();
    assert_eq!(run["startCatalogSyncRun"]["status"], "QUEUED");
    assert_eq!(run["startCatalogSyncRun"]["coverage"], "DISCOVERY_ONLY");
    assert_eq!(run["startCatalogSyncRun"]["startedFromRoot"], true);
    assert_eq!(run["startCatalogSyncRun"]["snapshotComplete"], false);
    assert_eq!(run["startCatalogSyncRun"]["observedCount"], "0");
    assert_eq!(run["startCatalogSyncRun"]["attemptCount"], "0");
    assert_eq!(run["startCatalogSyncRun"]["rowVersion"], "1");

    let polled = schema
        .execute(admin_request(format!(
            r#"{{ catalogSyncRun(runId: "{run_id}") {{ runId status observedCount rowVersion }} }}"#
        )))
        .await;
    assert!(polled.errors.is_empty(), "{:?}", polled.errors);
    assert_eq!(
        polled.data.into_json().unwrap()["catalogSyncRun"]["runId"],
        run_id.to_string()
    );

    let private_fields = schema
        .execute(admin_request(format!(
            r#"{{
                catalogSyncSource(sourceId: "{source_id}") {{
                    locator configurationDocument secretRef
                }}
                catalogSyncRun(runId: "{run_id}") {{
                    requestedCursor resultCursor errorMessage rawDocument parsedDocument
                }}
            }}"#
        )))
        .await;
    assert!(!private_fields.errors.is_empty());

    let worker_mutation = schema
        .execute(admin_request(format!(
            r#"mutation {{ claimCatalogSyncRun(runId: "{run_id}", expectedRowVersion: "1") }}"#
        )))
        .await;
    assert!(!worker_mutation.errors.is_empty());

    let unconfigured = test_schema().await;
    let unconfigured_artist = unconfigured
        .execute(admin_request(format!(
            r#"mutation {{
                createCatalogArtist(input: {{
                    artistId: "{artist_id}"
                    displayName: "Unconfigured Artist"
                }}) {{ artistId }}
            }}"#
        )))
        .await;
    assert!(unconfigured_artist.errors.is_empty());
    let rejected = unconfigured
        .execute(admin_request(format!(
            r#"mutation {{
                createCatalogSyncSource(input: {{
                    artistId: "{artist_id}"
                    kind: APPLE_MUSIC
                    locator: "1370700"
                    storefront: "jp"
                }}) {{ sourceId }}
            }}"#
        )))
        .await;
    assert_eq!(rejected.errors.len(), 1);
    assert_eq!(
        rejected.errors[0]
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.get("code")),
        Some(&async_graphql::Value::from(
            "CATALOG_SYNC_CREDENTIAL_NOT_CONFIGURED"
        ))
    );
    let empty = unconfigured
        .execute(admin_request(format!(
            r#"{{ catalogSyncSources(artistId: "{artist_id}") {{ sourceId }} }}"#
        )))
        .await;
    assert!(empty.errors.is_empty());
    assert!(empty.data.into_json().unwrap()["catalogSyncSources"]
        .as_array()
        .unwrap()
        .is_empty());
}
