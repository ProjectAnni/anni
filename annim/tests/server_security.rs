#![cfg(feature = "sqlite")]

use annim::{
    auth::AuthConfig,
    catalog::CatalogSyncService,
    config::{CatalogSyncProvisioningConfig, ServerConfig, APPLE_MUSIC_SECRET_REF_ENV},
    graphql::{build_schema, schema_builder_with_catalog_sync_service},
    migrator::Migrator,
    server::{build_router, ServerState},
};
use axum::{
    body::{to_bytes, Body},
    http::{
        header::{
            ACCESS_CONTROL_ALLOW_ORIGIN, ACCESS_CONTROL_REQUEST_HEADERS,
            ACCESS_CONTROL_REQUEST_METHOD, AUTHORIZATION, CACHE_CONTROL, CONTENT_TYPE, HOST,
            ORIGIN, WWW_AUTHENTICATE,
        },
        Method, Request, StatusCode,
    },
    Router,
};
use sea_orm::{prelude::Uuid, ConnectOptions, Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;
use tower::ServiceExt;

const TOKEN: &str = "0123456789abcdef0123456789abcdef";
const ALLOWED_ORIGIN: &str = "https://ui.example";
const WEB_APP: &str = include_str!("../../annim-web/app.js");

async fn connected_database() -> DatabaseConnection {
    let mut options = ConnectOptions::new("sqlite::memory:");
    options.max_connections(1);
    Database::connect(options).await.unwrap()
}

fn config(allowed_origins: Option<&str>, graphiql_enabled: bool) -> ServerConfig {
    ServerConfig::from_lookup(|name| match name {
        "ANNIM_ALLOWED_ORIGINS" => allowed_origins.map(str::to_owned),
        "ANNIM_GRAPHIQL_ENABLED" if graphiql_enabled => Some("true".to_owned()),
        _ => None,
    })
    .unwrap()
}

fn router(database: DatabaseConnection, config: &ServerConfig) -> Router {
    let schema = build_schema(database.clone());
    let auth = AuthConfig::new(TOKEN).unwrap();
    build_router(ServerState::new(schema, auth, database), config)
}

fn configured_catalog_router(database: DatabaseConnection, config: &ServerConfig) -> Router {
    let provisioning = CatalogSyncProvisioningConfig::from_lookup(|name| {
        (name == APPLE_MUSIC_SECRET_REF_ENV).then(|| "apple/developer-token.jwt".to_owned())
    })
    .unwrap();
    let catalog_sync_service =
        CatalogSyncService::with_provisioning(database.clone(), provisioning);
    let schema =
        schema_builder_with_catalog_sync_service(database.clone(), catalog_sync_service).finish();
    let auth = AuthConfig::new(TOKEN).unwrap();
    build_router(ServerState::new(schema, auth, database), config)
}

fn graphql_request(origin: Option<&str>, authorization: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder()
        .method(Method::POST)
        .uri("/")
        .header(CONTENT_TYPE, "application/json");
    if let Some(origin) = origin {
        builder = builder.header(ORIGIN, origin);
    }
    if let Some(authorization) = authorization {
        builder = builder.header(AUTHORIZATION, authorization);
    }
    builder
        .body(Body::from(r#"{"query":"{ __typename }"}"#))
        .unwrap()
}

fn web_query(name: &str) -> &str {
    let marker = format!("const {name} = `");
    let start = WEB_APP.find(&marker).expect("embedded query must exist") + marker.len();
    let remainder = &WEB_APP[start..];
    let end = remainder
        .find("`;\n")
        .expect("embedded query must end with a template literal");
    &remainder[..end]
}

async fn execute_web_graphql(
    app: &Router,
    query: &str,
    variables: serde_json::Value,
) -> serde_json::Value {
    let body = serde_json::json!({ "query": query, "variables": variables });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/")
                .header(HOST, "127.0.0.1:8000")
                .header(ORIGIN, "http://127.0.0.1:8000")
                .header(AUTHORIZATION, format!("Bearer {TOKEN}"))
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let response = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    serde_json::from_slice(&response).unwrap()
}

#[tokio::test]
async fn graphql_auth_debug_ui_and_health_have_fail_closed_transport_statuses() {
    let database = connected_database().await;
    let disabled = config(None, false);
    let app = router(database, &disabled);

    let unauthorized = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        unauthorized.headers().get(WWW_AUTHENTICATE).unwrap(),
        "Bearer realm=\"annim\""
    );

    let invalid = app
        .clone()
        .oneshot(graphql_request(
            None,
            Some("Bearer fedcba9876543210fedcba9876543210"),
        ))
        .await
        .unwrap();
    assert_eq!(invalid.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        invalid.headers().get(WWW_AUTHENTICATE).unwrap(),
        "Bearer realm=\"annim\""
    );

    let authorized = app
        .clone()
        .oneshot(graphql_request(None, Some(&format!("Bearer {TOKEN}"))))
        .await
        .unwrap();
    assert_eq!(authorized.status(), StatusCode::OK);

    let graphiql_disabled = app
        .clone()
        .oneshot(Request::get("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(graphiql_disabled.status(), StatusCode::METHOD_NOT_ALLOWED);

    let live = app
        .clone()
        .oneshot(Request::get("/health/live").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(live.status(), StatusCode::OK);
    assert_eq!(live.headers().get(CACHE_CONTROL).unwrap(), "no-store");
    let ready = app
        .clone()
        .oneshot(Request::get("/health/ready").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(ready.status(), StatusCode::OK);
    assert_eq!(ready.headers().get(CACHE_CONTROL).unwrap(), "no-store");

    let blocked_preflight = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/")
                .header(ORIGIN, "https://unconfigured.example")
                .header(ACCESS_CONTROL_REQUEST_METHOD, "POST")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(blocked_preflight.status(), StatusCode::FORBIDDEN);

    let enabled = config(None, true);
    let debug_app = router(DatabaseConnection::Disconnected, &enabled);
    let graphiql = debug_app
        .clone()
        .oneshot(Request::get("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(graphiql.status(), StatusCode::OK);
    let debug_post_without_auth = debug_app
        .clone()
        .oneshot(graphql_request(None, None))
        .await
        .unwrap();
    assert_eq!(debug_post_without_auth.status(), StatusCode::UNAUTHORIZED);
    let unavailable = debug_app
        .oneshot(Request::get("/health/ready").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(unavailable.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        unavailable.headers().get(CACHE_CONTROL).unwrap(),
        "no-store"
    );
}

#[tokio::test]
async fn exact_origin_policy_is_shared_by_http_cors_and_websocket_handshakes() {
    let database = connected_database().await;
    let config = config(Some(ALLOWED_ORIGIN), false);
    let app = router(database, &config);

    let disallowed_http = app
        .clone()
        .oneshot(graphql_request(
            Some("https://evil.example"),
            Some(&format!("Bearer {TOKEN}")),
        ))
        .await
        .unwrap();
    assert_eq!(disallowed_http.status(), StatusCode::FORBIDDEN);

    let disallowed_ws = app
        .clone()
        .oneshot(
            Request::get("/ws")
                .header(ORIGIN, "https://evil.example")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(disallowed_ws.status(), StatusCode::FORBIDDEN);

    let allowed_http = app
        .clone()
        .oneshot(graphql_request(
            Some(ALLOWED_ORIGIN),
            Some(&format!("Bearer {TOKEN}")),
        ))
        .await
        .unwrap();
    assert_eq!(allowed_http.status(), StatusCode::OK);
    assert_eq!(
        allowed_http
            .headers()
            .get(ACCESS_CONTROL_ALLOW_ORIGIN)
            .unwrap(),
        ALLOWED_ORIGIN
    );

    let preflight = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/")
                .header(ORIGIN, ALLOWED_ORIGIN)
                .header(ACCESS_CONTROL_REQUEST_METHOD, "POST")
                .header(ACCESS_CONTROL_REQUEST_HEADERS, "authorization,content-type")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(preflight.status(), StatusCode::OK);
    assert_eq!(
        preflight
            .headers()
            .get(ACCESS_CONTROL_ALLOW_ORIGIN)
            .unwrap(),
        ALLOWED_ORIGIN
    );

    let allowed_ws_route = app
        .oneshot(
            Request::get("/ws")
                .header(ORIGIN, ALLOWED_ORIGIN)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_ne!(allowed_ws_route.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn embedded_web_client_is_public_but_locked_to_same_origin_assets() {
    let app = router(connected_database().await, &config(None, false));

    for (path, content_type) in [
        ("/app", "text/html; charset=utf-8"),
        ("/app/", "text/html; charset=utf-8"),
        ("/app/styles.css", "text/css; charset=utf-8"),
        ("/app/app.js", "text/javascript; charset=utf-8"),
    ] {
        let response = app
            .clone()
            .oneshot(Request::get(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        assert_eq!(
            response.headers().get(CONTENT_TYPE).unwrap(),
            content_type,
            "{path}"
        );
        assert_eq!(response.headers().get(CACHE_CONTROL).unwrap(), "no-store");
        assert_eq!(
            response.headers().get("x-content-type-options").unwrap(),
            "nosniff"
        );
        let policy = response
            .headers()
            .get("content-security-policy")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(policy.contains("default-src 'none'"));
        assert!(policy.contains("script-src 'self'"));
        assert!(policy.contains("connect-src 'self'"));

        let body = to_bytes(response.into_body(), 2 * 1024 * 1024)
            .await
            .unwrap();
        assert!(!body.is_empty(), "{path}");
    }

    let missing = app
        .clone()
        .oneshot(Request::get("/app/unknown.js").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);

    let missing_host_graphql = app
        .clone()
        .oneshot(graphql_request(
            Some("http://127.0.0.1:8000"),
            Some(&format!("Bearer {TOKEN}")),
        ))
        .await
        .unwrap();
    assert_eq!(missing_host_graphql.status(), StatusCode::FORBIDDEN);

    let same_origin_graphql = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/")
                .header(HOST, "127.0.0.1:8000")
                .header(ORIGIN, "http://127.0.0.1:8000")
                .header(AUTHORIZATION, format!("Bearer {TOKEN}"))
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"query":"{ __typename }"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(same_origin_graphql.status(), StatusCode::OK);
}

#[tokio::test]
async fn embedded_client_queries_match_the_live_graphql_schema() {
    let database = connected_database().await;
    Migrator::up(&database, None).await.unwrap();
    let app = router(database, &config(None, false));
    let missing_id = Uuid::new_v4().to_string();

    for (name, variables) in [
        (
            "INTAKE_QUERY",
            serde_json::json!({ "limit": 1, "offset": 0 }),
        ),
        (
            "INGEST_REVIEW_QUERY",
            serde_json::json!({ "jobId": missing_id.clone() }),
        ),
        (
            "ARTISTS_QUERY",
            serde_json::json!({ "search": null, "limit": 1, "offset": 0 }),
        ),
        (
            "COLLECTION_QUERY",
            serde_json::json!({ "artistId": missing_id, "limit": 1, "offset": 0 }),
        ),
    ] {
        let response = execute_web_graphql(&app, web_query(name), variables).await;
        assert!(
            response.get("errors").is_none(),
            "{name} failed: {response}"
        );
    }
}

#[tokio::test]
async fn embedded_metadata_review_mutations_use_live_concurrency_contracts() {
    let database = connected_database().await;
    Migrator::up(&database, None).await.unwrap();
    let app = router(database, &config(None, false));
    let job_id = Uuid::new_v4().to_string();
    let candidate_id = Uuid::new_v4().to_string();
    let exact = " 曲名（Booklet） / 曲名(Booklet)・A〜B～C ";

    let created = execute_web_graphql(
        &app,
        "mutation WebTestCreate($jobId: UUID!) { createIngestJob(jobId: $jobId) { rowVersion } }",
        serde_json::json!({ "jobId": job_id.clone() }),
    )
    .await;
    assert_eq!(created["data"]["createIngestJob"]["rowVersion"], "1");

    let reviewing = execute_web_graphql(
        &app,
        web_query("EXECUTE_INGEST_COMMAND_MUTATION"),
        serde_json::json!({
            "input": {
                "jobId": job_id.clone(),
                "expectedRowVersion": "1",
                "command": { "beginReview": "EXECUTE" },
            }
        }),
    )
    .await;
    assert_eq!(
        reviewing["data"]["executeIngestJobCommand"]["rowVersion"],
        "2"
    );

    let configured = execute_web_graphql(
        &app,
        web_query("EDIT_METADATA_MUTATION"),
        serde_json::json!({
            "input": {
                "jobId": job_id.clone(),
                "expectedRowVersion": "2",
                "expectedRevision": "1",
                "edit": {
                    "configureReview": { "profile": "STREAMING", "trackCounts": [1] }
                },
            }
        }),
    )
    .await;
    assert_eq!(
        configured["data"]["editIngestMetadata"]["job"]["rowVersion"],
        "3"
    );

    let added = execute_web_graphql(
        &app,
        web_query("EDIT_METADATA_MUTATION"),
        serde_json::json!({
            "input": {
                "jobId": job_id.clone(),
                "expectedRowVersion": "3",
                "expectedRevision": "1",
                "edit": {
                    "addCandidate": {
                        "candidateId": candidate_id.clone(),
                        "field": { "scope": "ALBUM", "field": "TITLE" },
                        "value": { "text": exact },
                        "evidence": {
                            "sourceKind": "CD_BOOKLET",
                            "locator": "booklet.pdf#page=2",
                            "method": "MANUAL_TRANSCRIPTION",
                        },
                        "confidenceBasisPoints": 10000,
                    }
                },
            }
        }),
    )
    .await;
    assert_eq!(
        added["data"]["editIngestMetadata"]["job"]["rowVersion"],
        "4"
    );

    let accepted = execute_web_graphql(
        &app,
        web_query("EDIT_METADATA_MUTATION"),
        serde_json::json!({
            "input": {
                "jobId": job_id.clone(),
                "expectedRowVersion": "4",
                "expectedRevision": "1",
                "edit": { "acceptCandidate": { "candidateId": candidate_id } },
            }
        }),
    )
    .await;
    assert_eq!(
        accepted["data"]["editIngestMetadata"]["job"]["rowVersion"],
        "5"
    );

    let review = execute_web_graphql(
        &app,
        web_query("INGEST_REVIEW_QUERY"),
        serde_json::json!({ "jobId": job_id.clone() }),
    )
    .await;
    let candidate = &review["data"]["ingestMetadataDraft"]["draft"]["candidates"][0];
    assert_eq!(candidate["value"]["text"], exact);
    assert_eq!(candidate["decision"], "ACCEPTED");

    let incomplete = execute_web_graphql(
        &app,
        web_query("APPROVE_METADATA_MUTATION"),
        serde_json::json!({
            "input": {
                "jobId": job_id.clone(),
                "expectedRowVersion": "5",
                "expectedRevision": "1",
            }
        }),
    )
    .await;
    assert_eq!(
        incomplete["errors"][0]["extensions"]["code"],
        "INGEST_METADATA_INCOMPLETE"
    );

    let revised = execute_web_graphql(
        &app,
        web_query("REVISE_METADATA_MUTATION"),
        serde_json::json!({
            "input": {
                "jobId": job_id,
                "expectedRowVersion": "5",
                "expectedRevision": "1",
            }
        }),
    )
    .await;
    assert_eq!(
        revised["data"]["reviseIngestMetadata"]["job"]["rowVersion"],
        "6"
    );
    assert_eq!(
        revised["data"]["reviseIngestMetadata"]["job"]["metadataRevision"],
        "2"
    );
    assert_eq!(
        revised["data"]["reviseIngestMetadata"]["draft"]["revision"],
        "2"
    );
}

#[tokio::test]
async fn embedded_catalog_authoring_mutations_preserve_exact_artist_and_release_text() {
    let database = connected_database().await;
    Migrator::up(&database, None).await.unwrap();
    let app = router(database, &config(None, false));
    let exact_artist = " Artist（公式）・A〜B～C ";
    let exact_title = " 作品・A〜B～C（初回） ";

    let created_artist = execute_web_graphql(
        &app,
        web_query("CREATE_CATALOG_ARTIST_MUTATION"),
        serde_json::json!({
            "input": {
                "displayName": exact_artist,
                "sortName": "Artist",
                "notes": "official spelling",
            }
        }),
    )
    .await;
    assert!(created_artist.get("errors").is_none(), "{created_artist}");
    let artist = &created_artist["data"]["createCatalogArtist"];
    assert_eq!(artist["displayName"], exact_artist);
    assert_eq!(artist["rowVersion"], "1");
    let artist_id = artist["artistId"].as_str().unwrap().to_owned();

    let created_release = execute_web_graphql(
        &app,
        web_query("CREATE_CATALOG_RELEASE_MUTATION"),
        serde_json::json!({
            "input": {
                "artistId": artist_id.clone(),
                "title": exact_title,
                "edition": "初回限定盤（CD＋BD）",
                "catalog": "TEST-0001",
                "releaseDate": "2026-07",
                "kind": "ALBUM",
                "notes": "manual catalog entry",
            }
        }),
    )
    .await;
    assert!(created_release.get("errors").is_none(), "{created_release}");
    let release = &created_release["data"]["createCatalogRelease"];
    assert_eq!(release["title"], exact_title);
    assert_eq!(release["collectionState"], "MISSING");
    assert_eq!(release["rowVersion"], "1");
    let release_id = release["releaseId"].as_str().unwrap().to_owned();

    let wanted = execute_web_graphql(
        &app,
        web_query("EXECUTE_CATALOG_RELEASE_COMMAND_MUTATION"),
        serde_json::json!({
            "input": {
                "releaseId": release_id.clone(),
                "expectedRowVersion": "1",
                "command": { "markWanted": "EXECUTE" },
            }
        }),
    )
    .await;
    assert!(wanted.get("errors").is_none(), "{wanted}");
    assert_eq!(
        wanted["data"]["executeCatalogReleaseCommand"]["collectionState"],
        "WANTED"
    );
    assert_eq!(
        wanted["data"]["executeCatalogReleaseCommand"]["rowVersion"],
        "2"
    );

    let exact_source = " 天使动漫（群友补档） ";
    let recorded = execute_web_graphql(
        &app,
        web_query("EXECUTE_CATALOG_RELEASE_COMMAND_MUTATION"),
        serde_json::json!({
            "input": {
                "releaseId": release_id,
                "expectedRowVersion": "2",
                "command": {
                    "recordCopy": {
                        "sourceKind": "ANGEL_ANIME",
                        "sourceLabel": exact_source,
                        "privateLocator": "私有/不可公开/PT种子",
                        "codec": "FLAC",
                        "sampleRateHz": 96000,
                        "bitDepth": 24,
                        "channels": 2,
                        "trackCount": 12,
                        "byteLength": "1234567890",
                        "manifestDigest": "5a".repeat(32),
                        "qualityVerified": true,
                        "notes": "原始抓取・A〜B～C",
                    }
                },
            }
        }),
    )
    .await;
    assert!(recorded.get("errors").is_none(), "{recorded}");
    let recorded_release = &recorded["data"]["executeCatalogReleaseCommand"];
    assert_eq!(recorded_release["collectionState"], "ACQUIRED");
    assert_eq!(recorded_release["rowVersion"], "3");
    assert_eq!(recorded_release["copies"][0]["sourceLabel"], exact_source);
    assert_eq!(
        recorded_release["copies"][0]["qualityTier"],
        "HI_RES_LOSSLESS"
    );
    assert_eq!(recorded_release["copies"][0]["byteLength"], "1234567890");
    assert_eq!(recorded_release["copies"][0]["qualityVerified"], true);
    assert!(
        recorded_release["copies"][0]
            .as_object()
            .unwrap()
            .get("privateLocator")
            .is_none(),
        "private locator must not be exposed by the embedded client mutation"
    );

    let collection = execute_web_graphql(
        &app,
        web_query("COLLECTION_QUERY"),
        serde_json::json!({ "artistId": artist_id, "limit": 10, "offset": 0 }),
    )
    .await;
    assert_eq!(
        collection["data"]["catalogArtistCollection"]["artist"]["displayName"],
        exact_artist
    );
    assert_eq!(
        collection["data"]["catalogArtistCollection"]["releases"][0]["title"],
        exact_title
    );
    let collection = &collection["data"]["catalogArtistCollection"];
    assert_eq!(collection["summary"]["acquired"], "1");
    assert_eq!(collection["summary"]["collected"], "1");
    assert_eq!(collection["releases"][0]["collectionState"], "ACQUIRED");
    assert_eq!(
        collection["releases"][0]["copies"][0]["sourceLabel"],
        exact_source
    );
    assert_eq!(
        collection["releases"][0]["copies"][0]["qualityTier"],
        "HI_RES_LOSSLESS"
    );
}

#[tokio::test]
async fn embedded_catalog_sync_controls_follow_the_live_provisioning_and_concurrency_contract() {
    let database = connected_database().await;
    Migrator::up(&database, None).await.unwrap();
    let app = configured_catalog_router(database, &config(None, false));

    let created_artist = execute_web_graphql(
        &app,
        web_query("CREATE_CATALOG_ARTIST_MUTATION"),
        serde_json::json!({
            "input": {
                "displayName": "Catalog Sync Test Artist",
            }
        }),
    )
    .await;
    assert!(created_artist.get("errors").is_none(), "{created_artist}");
    let artist_id = created_artist["data"]["createCatalogArtist"]["artistId"]
        .as_str()
        .unwrap()
        .to_owned();

    let created_source = execute_web_graphql(
        &app,
        web_query("CREATE_CATALOG_SYNC_SOURCE_MUTATION"),
        serde_json::json!({
            "input": {
                "artistId": artist_id.clone(),
                "kind": "APPLE_MUSIC",
                "locator": "1370700",
                "storefront": "jp",
                "locale": "ja-JP",
            }
        }),
    )
    .await;
    assert!(created_source.get("errors").is_none(), "{created_source}");
    let source = &created_source["data"]["createCatalogSyncSource"];
    assert_eq!(source["provisioningState"], "READY_TO_QUEUE");
    assert_eq!(source["rowVersion"], "1");
    assert_eq!(source["enabled"], true);
    let source_id = source["sourceId"].as_str().unwrap().to_owned();

    let sources = execute_web_graphql(
        &app,
        web_query("CATALOG_SYNC_SOURCES_QUERY"),
        serde_json::json!({ "artistId": artist_id }),
    )
    .await;
    assert!(sources.get("errors").is_none(), "{sources}");
    let sources = sources["data"]["catalogSyncSources"].as_array().unwrap();
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0]["sourceId"], source_id);
    assert_eq!(sources[0]["rowVersion"], "1");

    let source_by_id = execute_web_graphql(
        &app,
        web_query("CATALOG_SYNC_SOURCE_QUERY"),
        serde_json::json!({ "sourceId": source_id.clone() }),
    )
    .await;
    assert!(source_by_id.get("errors").is_none(), "{source_by_id}");
    assert_eq!(
        source_by_id["data"]["catalogSyncSource"]["sourceId"],
        source_id
    );

    let started = execute_web_graphql(
        &app,
        web_query("START_CATALOG_SYNC_RUN_MUTATION"),
        serde_json::json!({ "input": { "sourceId": source_id.clone() } }),
    )
    .await;
    assert!(started.get("errors").is_none(), "{started}");
    let run = &started["data"]["startCatalogSyncRun"];
    assert_eq!(run["sourceId"], source_id);
    assert_eq!(run["status"], "QUEUED");
    assert_eq!(run["rowVersion"], "1");
    let run_id = run["runId"].as_str().unwrap().to_owned();

    let run_by_id = execute_web_graphql(
        &app,
        web_query("CATALOG_SYNC_RUN_QUERY"),
        serde_json::json!({ "runId": run_id.clone() }),
    )
    .await;
    assert!(run_by_id.get("errors").is_none(), "{run_by_id}");
    assert_eq!(run_by_id["data"]["catalogSyncRun"]["runId"], run_id);
    assert_eq!(run_by_id["data"]["catalogSyncRun"]["status"], "QUEUED");

    let sources_after_enqueue = execute_web_graphql(
        &app,
        web_query("CATALOG_SYNC_SOURCES_QUERY"),
        serde_json::json!({ "artistId": artist_id }),
    )
    .await;
    assert!(
        sources_after_enqueue.get("errors").is_none(),
        "{sources_after_enqueue}"
    );
    let source_after_enqueue = &sources_after_enqueue["data"]["catalogSyncSources"][0];
    assert_eq!(source_after_enqueue["sourceId"], source_id);
    assert_eq!(source_after_enqueue["rowVersion"], "2");

    let history = execute_web_graphql(
        &app,
        web_query("CATALOG_SYNC_RUNS_QUERY"),
        serde_json::json!({
            "sourceId": source_id.clone(),
            "limit": 10,
            "offset": 0,
        }),
    )
    .await;
    assert!(history.get("errors").is_none(), "{history}");
    let runs = history["data"]["catalogSyncRuns"].as_array().unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0]["runId"], run_id);
    assert_eq!(runs[0]["sourceId"], source_id);
    assert_eq!(runs[0]["status"], "QUEUED");

    let disabled = execute_web_graphql(
        &app,
        web_query("SET_CATALOG_SYNC_SOURCE_ENABLED_MUTATION"),
        serde_json::json!({
            "input": {
                "sourceId": source_id.clone(),
                "expectedRowVersion": "2",
                "enabled": false,
            }
        }),
    )
    .await;
    assert!(disabled.get("errors").is_none(), "{disabled}");
    let disabled = &disabled["data"]["setCatalogSyncSourceEnabled"];
    assert_eq!(disabled["enabled"], false);
    assert_eq!(disabled["provisioningState"], "DISABLED");
    assert_eq!(disabled["rowVersion"], "3");

    let stale_reenable = execute_web_graphql(
        &app,
        web_query("SET_CATALOG_SYNC_SOURCE_ENABLED_MUTATION"),
        serde_json::json!({
            "input": {
                "sourceId": source_id,
                "expectedRowVersion": "2",
                "enabled": true,
            }
        }),
    )
    .await;
    assert_eq!(
        stale_reenable["errors"][0]["extensions"]["code"],
        "CATALOG_SYNC_SOURCE_CONFLICT"
    );
}
