#![cfg(feature = "sqlite")]

use annim::{
    auth::AuthConfig,
    config::ServerConfig,
    graphql::build_schema,
    server::{build_router, ServerState},
};
use axum::{
    body::Body,
    http::{
        header::{
            ACCESS_CONTROL_ALLOW_ORIGIN, ACCESS_CONTROL_REQUEST_HEADERS,
            ACCESS_CONTROL_REQUEST_METHOD, AUTHORIZATION, CACHE_CONTROL, CONTENT_TYPE, ORIGIN,
            WWW_AUTHENTICATE,
        },
        Method, Request, StatusCode,
    },
    Router,
};
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use tower::ServiceExt;

const TOKEN: &str = "0123456789abcdef0123456789abcdef";
const ALLOWED_ORIGIN: &str = "https://ui.example";

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
