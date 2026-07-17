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

#[tokio::test]
async fn graphql_manages_cover_candidates_without_exposing_worker_secrets() {
    let schema = test_schema().await;
    let artist_id = Uuid::new_v4();
    let release_id = Uuid::new_v4();
    let candidate_id = Uuid::new_v4();

    let seeded = schema
        .execute(admin_request(format!(
            r#"mutation {{
                createCatalogArtist(input: {{
                    artistId: "{artist_id}"
                    displayName: "Artist（公式）"
                }}) {{ artistId }}
                createCatalogRelease(input: {{
                    releaseId: "{release_id}"
                    artistId: "{artist_id}"
                    title: "作品・A〜B～C (初回)"
                    kind: ALBUM
                }}) {{ releaseId }}
            }}"#
        )))
        .await;
    assert!(seeded.errors.is_empty(), "{:?}", seeded.errors);

    let added = schema
        .execute(admin_request(format!(
            r#"mutation {{
                addCoverCandidate(input: {{
                    candidateId: "{candidate_id}"
                    releaseId: "{release_id}"
                    discNumber: 0
                    sourceKind: AMAZON
                    submittedUrl: "https://m.media-amazon.com/images/I/example._SL500_.jpg"
                }}) {{
                    candidateId state sourceKind hasRemoteUrl attemptCount rowVersion asset {{ assetId }}
                }}
            }}"#
        )))
        .await;
    assert!(added.errors.is_empty(), "{:?}", added.errors);
    let added = added.data.into_json().unwrap()["addCoverCandidate"].clone();
    assert_eq!(added["candidateId"], candidate_id.to_string());
    assert_eq!(added["state"], "DISCOVERED");
    assert_eq!(added["sourceKind"], "AMAZON");
    assert_eq!(added["hasRemoteUrl"], true);
    assert_eq!(added["attemptCount"], "0");
    assert_eq!(added["rowVersion"], "1");
    assert!(added["asset"].is_null());

    let queued = schema
        .execute(admin_request(format!(
            r#"mutation {{
                queueCoverCandidate(input: {{
                    candidateId: "{candidate_id}"
                    expectedRowVersion: "1"
                }}) {{ state rowVersion }}
            }}"#
        )))
        .await;
    assert!(queued.errors.is_empty(), "{:?}", queued.errors);
    let queued = queued.data.into_json().unwrap();
    assert_eq!(queued["queueCoverCandidate"]["state"], "QUEUED");
    assert_eq!(queued["queueCoverCandidate"]["rowVersion"], "2");

    let listed = schema
        .execute(admin_request(format!(
            r#"{{ coverCandidates(releaseId: "{release_id}", discNumber: 0) {{
                candidateId state hasRemoteUrl rowVersion
            }} }}"#
        )))
        .await;
    assert!(listed.errors.is_empty(), "{:?}", listed.errors);
    let listed = listed.data.into_json().unwrap();
    assert_eq!(
        listed["coverCandidates"][0]["candidateId"],
        candidate_id.to_string()
    );
    assert_eq!(listed["coverCandidates"][0]["state"], "QUEUED");

    let sensitive_fields = schema
        .execute(admin_request(format!(
            r#"{{ coverCandidates(releaseId: "{release_id}") {{
                submittedUrl requestUrl storageKey leaseToken
            }} }}"#
        )))
        .await;
    assert!(!sensitive_fields.errors.is_empty());
}
