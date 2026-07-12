use std::sync::Arc;

use async_graphql::http::{graphiql_source, ALL_WEBSOCKET_PROTOCOLS};
use async_graphql_axum::{GraphQLProtocol, GraphQLRequest, GraphQLResponse, GraphQLWebSocket};
use axum::{
    extract::{Request, State, WebSocketUpgrade},
    http::{
        header::{AUTHORIZATION, CONTENT_TYPE, ORIGIN, WWW_AUTHENTICATE},
        HeaderMap, HeaderValue, Method, StatusCode,
    },
    middleware::{self, Next},
    response::{self, IntoResponse, Response},
    routing::{get, post},
    Extension, Json, Router,
};
use sea_orm::DatabaseConnection;
use serde::Serialize;
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::{
    auth::{on_connection_init, AuthConfig, AuthenticatedAdmin},
    config::ServerConfig,
    graphql::MetadataSchema,
};

const BEARER_CHALLENGE: HeaderValue = HeaderValue::from_static("Bearer realm=\"annim\"");
const NO_STORE: HeaderValue = HeaderValue::from_static("no-store");

#[derive(Clone)]
pub struct ServerState {
    schema: MetadataSchema,
    auth: AuthConfig,
    database: DatabaseConnection,
}

impl ServerState {
    pub fn new(schema: MetadataSchema, auth: AuthConfig, database: DatabaseConnection) -> Self {
        Self {
            schema,
            auth,
            database,
        }
    }
}

#[derive(Clone)]
struct OriginPolicy {
    allowed: Arc<[HeaderValue]>,
}

impl OriginPolicy {
    fn from_config(config: &ServerConfig) -> Self {
        Self {
            allowed: config.allowed_origins().to_vec().into(),
        }
    }

    fn accepts(&self, headers: &HeaderMap) -> bool {
        let mut values = headers.get_all(ORIGIN).iter();
        match (values.next(), values.next()) {
            (None, None) => true,
            (Some(origin), None) => self.allowed.iter().any(|allowed| allowed == origin),
            _ => false,
        }
    }
}

pub fn build_router(state: ServerState, config: &ServerConfig) -> Router {
    let auth_layer = middleware::from_fn_with_state(state.auth.clone(), require_http_admin);
    let mut root = post(graphql_handler).route_layer(auth_layer);
    if config.graphiql_enabled() {
        root = root.get(graphql_playground);
    }

    let allowed_origins = config.allowed_origins().to_vec();
    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([AUTHORIZATION, CONTENT_TYPE])
        .allow_origin(AllowOrigin::list(allowed_origins));
    let origin_policy = OriginPolicy::from_config(config);

    Router::new()
        .route("/", root)
        .route("/ws", get(graphql_ws_handler))
        .route("/health/live", get(liveness))
        .route("/health/ready", get(readiness))
        .with_state(state)
        .layer(cors)
        // CORS controls browser response headers; this outer policy also
        // rejects disallowed WebSocket handshakes and non-preflight requests.
        .layer(middleware::from_fn_with_state(
            origin_policy,
            enforce_origin,
        ))
}

async fn require_http_admin(
    State(auth): State<AuthConfig>,
    mut request: Request,
    next: Next,
) -> Response {
    let authenticated = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| auth.authenticate_bearer(value).ok());

    match authenticated {
        Some(admin) => {
            request.extensions_mut().insert(admin);
            next.run(request).await
        }
        None => unauthorized_response(),
    }
}

async fn enforce_origin(
    State(policy): State<OriginPolicy>,
    request: Request,
    next: Next,
) -> Response {
    if policy.accepts(request.headers()) {
        next.run(request).await
    } else {
        (StatusCode::FORBIDDEN, "Origin not allowed").into_response()
    }
}

fn unauthorized_response() -> Response {
    let mut response = (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    response
        .headers_mut()
        .insert(WWW_AUTHENTICATE, BEARER_CHALLENGE);
    response
}

async fn graphql_handler(
    State(state): State<ServerState>,
    Extension(admin): Extension<AuthenticatedAdmin>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    state
        .schema
        .execute(req.into_inner().data(admin))
        .await
        .into()
}

async fn graphql_ws_handler(
    State(state): State<ServerState>,
    protocol: GraphQLProtocol,
    websocket: WebSocketUpgrade,
) -> Response {
    websocket
        .protocols(ALL_WEBSOCKET_PROTOCOLS)
        .on_upgrade(move |stream| {
            let auth = state.auth.clone();
            GraphQLWebSocket::new(stream, state.schema, protocol)
                .on_connection_init(move |value| on_connection_init(auth.clone(), value))
                .serve()
        })
}

async fn graphql_playground() -> impl IntoResponse {
    response::Html(graphiql_source("/", Some("/ws")))
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

async fn liveness() -> Response {
    health_response(StatusCode::OK, "live")
}

async fn readiness(State(state): State<ServerState>) -> Response {
    if state.database.ping().await.is_ok() {
        health_response(StatusCode::OK, "ready")
    } else {
        health_response(StatusCode::SERVICE_UNAVAILABLE, "unavailable")
    }
}

fn health_response(status: StatusCode, health_status: &'static str) -> Response {
    let mut response = (
        status,
        Json(HealthResponse {
            status: health_status,
        }),
    )
        .into_response();
    response
        .headers_mut()
        .insert(axum::http::header::CACHE_CONTROL, NO_STORE);
    response
}
