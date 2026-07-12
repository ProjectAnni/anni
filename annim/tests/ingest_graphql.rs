#![cfg(feature = "sqlite")]

use annim::{auth::AuthConfig, graphql::build_schema, migrator::Migrator};
use async_graphql::Request;
use sea_orm::{prelude::Uuid, ConnectOptions, Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;
use tokio::time::{timeout, Duration};
use tokio_stream::StreamExt;

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
async fn ingest_queries_require_an_admin_token() {
    let schema = test_schema().await;
    let response = schema.execute("{ ingestJobs { jobId } }").await;

    assert_eq!(response.errors.len(), 1);
    assert_eq!(response.errors[0].message, "Unauthorized");
}

#[tokio::test]
async fn graphql_executes_commands_and_reports_stale_writers() {
    let schema = test_schema().await;
    let job_id = Uuid::new_v4();

    let created = schema
        .execute(admin_request(format!(
            r#"mutation {{
                createIngestJob(jobId: "{job_id}") {{
                    jobId state metadataRevision rowVersion
                }}
            }}"#
        )))
        .await;
    assert!(created.errors.is_empty(), "{:?}", created.errors);
    assert_eq!(
        created.data.into_json().unwrap()["createIngestJob"]["rowVersion"],
        "1"
    );

    let reviewed = schema
        .execute(admin_request(format!(
            r#"mutation {{
                executeIngestJobCommand(input: {{
                    jobId: "{job_id}"
                    expectedRowVersion: "1"
                    command: {{ beginReview: EXECUTE }}
                }}) {{ state rowVersion }}
            }}"#
        )))
        .await;
    assert!(reviewed.errors.is_empty(), "{:?}", reviewed.errors);
    let reviewed = reviewed.data.into_json().unwrap();
    assert_eq!(reviewed["executeIngestJobCommand"]["state"], "REVIEWING");
    assert_eq!(reviewed["executeIngestJobCommand"]["rowVersion"], "2");

    let stale = schema
        .execute(admin_request(format!(
            r#"mutation {{
                executeIngestJobCommand(input: {{
                    jobId: "{job_id}"
                    expectedRowVersion: "1"
                    command: {{ cancel: EXECUTE }}
                }}) {{ state rowVersion }}
            }}"#
        )))
        .await;
    assert_eq!(
        error_code(&stale),
        Some(&async_graphql::Value::from("INGEST_JOB_CONFLICT"))
    );
    let extensions = stale.errors[0].extensions.as_ref().unwrap();
    assert_eq!(
        extensions.get("expectedRowVersion"),
        Some(&async_graphql::Value::from("1"))
    );
    assert_eq!(
        extensions.get("actualRowVersion"),
        Some(&async_graphql::Value::from("2"))
    );
}

#[tokio::test]
async fn graphql_exposes_domain_transition_errors_with_stable_codes() {
    let schema = test_schema().await;
    let job_id = Uuid::new_v4();
    let create = schema
        .execute(admin_request(format!(
            r#"mutation {{ createIngestJob(jobId: "{job_id}") {{ jobId }} }}"#
        )))
        .await;
    assert!(create.errors.is_empty(), "{:?}", create.errors);

    let digest = "00".repeat(32);
    let response = schema
        .execute(admin_request(format!(
            r#"mutation {{
                executeIngestJobCommand(input: {{
                    jobId: "{job_id}"
                    expectedRowVersion: "1"
                    command: {{ beginExecution: {{ planDigest: "{digest}" }} }}
                }}) {{ state }}
            }}"#
        )))
        .await;

    assert_eq!(
        error_code(&response),
        Some(&async_graphql::Value::from("INGEST_INVALID_TRANSITION"))
    );
}

#[tokio::test]
async fn subscription_emits_only_after_a_committed_update() {
    let schema = test_schema().await;
    let job_id = Uuid::new_v4();
    let create = schema
        .execute(admin_request(format!(
            r#"mutation {{ createIngestJob(jobId: "{job_id}") {{ jobId }} }}"#
        )))
        .await;
    assert!(create.errors.is_empty(), "{:?}", create.errors);

    let mut events = Box::pin(schema.execute_stream(admin_request(format!(
        r#"subscription {{
            ingestJobChanged(jobId: "{job_id}", afterRowVersion: "1") {{
                state rowVersion
            }}
        }}"#
    ))));

    // Poll once so the resolver installs its receiver before the mutation.
    assert!(timeout(Duration::from_millis(20), events.next())
        .await
        .is_err());

    let mutation = schema
        .execute(admin_request(format!(
            r#"mutation {{
                executeIngestJobCommand(input: {{
                    jobId: "{job_id}"
                    expectedRowVersion: "1"
                    command: {{ beginReview: EXECUTE }}
                }}) {{ rowVersion }}
            }}"#
        )))
        .await;
    assert!(mutation.errors.is_empty(), "{:?}", mutation.errors);

    let event = timeout(Duration::from_secs(1), events.next())
        .await
        .expect("subscription timed out")
        .expect("subscription ended");
    assert!(event.errors.is_empty(), "{:?}", event.errors);
    let event = event.data.into_json().unwrap();
    assert_eq!(event["ingestJobChanged"]["state"], "REVIEWING");
    assert_eq!(event["ingestJobChanged"]["rowVersion"], "2");
}

#[tokio::test]
async fn graphql_reviews_exact_metadata_with_typed_evidence_and_optimistic_locking() {
    let schema = test_schema().await;
    let job_id = Uuid::new_v4();
    let candidate_id = Uuid::new_v4();
    let exact = "曲名（Booklet） / 曲名(Booklet)・A〜B～C";

    let created = schema
        .execute(admin_request(format!(
            r#"mutation {{ createIngestJob(jobId: "{job_id}") {{ rowVersion }} }}"#
        )))
        .await;
    assert!(created.errors.is_empty(), "{:?}", created.errors);
    let reviewing = schema
        .execute(admin_request(format!(
            r#"mutation {{
                executeIngestJobCommand(input: {{
                    jobId: "{job_id}"
                    expectedRowVersion: "1"
                    command: {{ beginReview: EXECUTE }}
                }}) {{ rowVersion }}
            }}"#
        )))
        .await;
    assert!(reviewing.errors.is_empty(), "{:?}", reviewing.errors);

    let configured = schema
        .execute(admin_request(format!(
            r#"mutation {{
                editIngestMetadata(input: {{
                    jobId: "{job_id}"
                    expectedRowVersion: "2"
                    expectedRevision: "1"
                    edit: {{ configureReview: {{ profile: CD, trackCounts: [1] }} }}
                }}) {{
                    job {{ rowVersion }}
                    draft {{ profile trackCounts requirementsConfigured totalRequired complete }}
                }}
            }}"#
        )))
        .await;
    assert!(configured.errors.is_empty(), "{:?}", configured.errors);
    let configured = configured.data.into_json().unwrap();
    assert_eq!(configured["editIngestMetadata"]["job"]["rowVersion"], "3");
    assert_eq!(configured["editIngestMetadata"]["draft"]["profile"], "CD");
    assert_eq!(
        configured["editIngestMetadata"]["draft"]["totalRequired"],
        "8"
    );
    assert_eq!(configured["editIngestMetadata"]["draft"]["complete"], false);

    let incomplete = schema
        .execute(admin_request(format!(
            r#"mutation {{
                approveIngestMetadata(input: {{
                    jobId: "{job_id}"
                    expectedRowVersion: "3"
                    expectedRevision: "1"
                }}) {{ job {{ rowVersion }} }}
            }}"#
        )))
        .await;
    assert_eq!(
        error_code(&incomplete),
        Some(&async_graphql::Value::from("INGEST_METADATA_INCOMPLETE"))
    );

    let wrong_value = schema
        .execute(admin_request(format!(
            r#"mutation {{
                editIngestMetadata(input: {{
                    jobId: "{job_id}"
                    expectedRowVersion: "3"
                    expectedRevision: "1"
                    edit: {{ addCandidate: {{
                        field: {{ scope: ALBUM, field: TITLE }}
                        value: {{ trackType: NORMAL }}
                        evidence: {{
                            sourceKind: CD_BOOKLET
                            locator: "booklet.pdf#page=2"
                            method: MANUAL_TRANSCRIPTION
                        }}
                        confidenceBasisPoints: 10000
                    }} }}
                }}) {{ job {{ rowVersion }} }}
            }}"#
        )))
        .await;
    assert_eq!(
        error_code(&wrong_value),
        Some(&async_graphql::Value::from("INGEST_METADATA_INVALID_VALUE"))
    );

    let added = schema
        .execute(admin_request(format!(
            r#"mutation {{
                editIngestMetadata(input: {{
                    jobId: "{job_id}"
                    expectedRowVersion: "3"
                    expectedRevision: "1"
                    edit: {{ addCandidate: {{
                        candidateId: "{candidate_id}"
                        field: {{ scope: ALBUM, field: TITLE }}
                        value: {{ text: "{exact}" }}
                        evidence: {{
                            sourceKind: CD_BOOKLET
                            locator: "booklet.pdf#page=2"
                            detail: "front cover title"
                            method: MANUAL_TRANSCRIPTION
                        }}
                        confidenceBasisPoints: 10000
                    }} }}
                }}) {{
                    job {{ rowVersion }}
                    draft {{
                        candidates {{
                            candidateId recommended decision
                            value {{ kind text }}
                            evidence {{ sourceKind locator method }}
                        }}
                    }}
                }}
            }}"#
        )))
        .await;
    assert!(added.errors.is_empty(), "{:?}", added.errors);
    let added = added.data.into_json().unwrap();
    assert_eq!(added["editIngestMetadata"]["job"]["rowVersion"], "4");
    let candidate = &added["editIngestMetadata"]["draft"]["candidates"][0];
    assert_eq!(candidate["value"]["text"], exact);
    assert_eq!(candidate["decision"], "PENDING");
    assert_eq!(candidate["recommended"], true);
    assert_eq!(candidate["evidence"]["sourceKind"], "CD_BOOKLET");

    let accepted = schema
        .execute(admin_request(format!(
            r#"mutation {{
                editIngestMetadata(input: {{
                    jobId: "{job_id}"
                    expectedRowVersion: "4"
                    expectedRevision: "1"
                    edit: {{ acceptCandidate: {{ candidateId: "{candidate_id}" }} }}
                }}) {{ job {{ rowVersion }} draft {{ acceptedRequired }} }}
            }}"#
        )))
        .await;
    assert!(accepted.errors.is_empty(), "{:?}", accepted.errors);
    assert_eq!(
        accepted.data.into_json().unwrap()["editIngestMetadata"]["job"]["rowVersion"],
        "5"
    );

    let queried = schema
        .execute(admin_request(format!(
            r#"query {{
                ingestMetadataDraft(jobId: "{job_id}") {{
                    job {{ rowVersion }}
                    draft {{
                        revision acceptedRequired
                        candidates {{ candidateId decision value {{ text }} }}
                    }}
                }}
                ingestMetadataRevisions(jobId: "{job_id}") {{ revision }}
            }}"#
        )))
        .await;
    assert!(queried.errors.is_empty(), "{:?}", queried.errors);
    let queried = queried.data.into_json().unwrap();
    assert_eq!(queried["ingestMetadataDraft"]["job"]["rowVersion"], "5");
    assert_eq!(queried["ingestMetadataRevisions"][0]["revision"], "1");
    assert_eq!(
        queried["ingestMetadataDraft"]["draft"]["candidates"][0]["decision"],
        "ACCEPTED"
    );
    assert_eq!(
        queried["ingestMetadataDraft"]["draft"]["candidates"][0]["value"]["text"],
        exact
    );

    let stale = schema
        .execute(admin_request(format!(
            r#"mutation {{
                editIngestMetadata(input: {{
                    jobId: "{job_id}"
                    expectedRowVersion: "4"
                    expectedRevision: "1"
                    edit: {{ rejectCandidate: {{ candidateId: "{candidate_id}" }} }}
                }}) {{ job {{ rowVersion }} }}
            }}"#
        )))
        .await;
    assert_eq!(
        error_code(&stale),
        Some(&async_graphql::Value::from("INGEST_JOB_CONFLICT"))
    );
}
