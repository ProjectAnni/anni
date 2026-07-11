#![cfg(feature = "sqlite")]

use annim::{auth::AuthToken, graphql::build_schema, migrator::Migrator};
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
    let token = std::env::var("ANNIM_AUTH_TOKEN").unwrap_or_else(|_| "114514".to_owned());
    Request::new(query).data(AuthToken::new(token))
}

fn error_code(response: &async_graphql::Response) -> Option<&async_graphql::Value> {
    response.errors.first()?.extensions.as_ref()?.get("code")
}

#[tokio::test]
async fn ingest_queries_require_an_admin_token() {
    let schema = test_schema().await;
    let response = schema.execute("{ ingestJobs { jobId } }").await;

    assert_eq!(response.errors.len(), 1);
    assert_eq!(response.errors[0].message, "Token is required");
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
