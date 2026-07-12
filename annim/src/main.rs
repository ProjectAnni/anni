use annim::{
    auth::{on_connection_init, AuthConfig},
    graphql::{schema_builder, MetadataSchema},
    search::RepositorySearchManager,
};
use async_graphql::http::{graphiql_source, ALL_WEBSOCKET_PROTOCOLS};
use async_graphql_axum::{GraphQLProtocol, GraphQLRequest, GraphQLResponse, GraphQLWebSocket};
use axum::{
    extract::{State, WebSocketUpgrade},
    http::{HeaderMap, Method},
    response::{self, IntoResponse, Response},
    routing::get,
    Router,
};
use sea_orm::Database;
use sea_orm_migration::MigratorTrait;
use tokio::net::TcpListener;
use tower_http::cors;
use tower_http::cors::CorsLayer;

async fn graphql_playground() -> impl IntoResponse {
    response::Html(graphiql_source("/", Some("/ws")))
}

#[derive(Clone)]
struct AppState {
    schema: MetadataSchema,
    auth: AuthConfig,
}

fn authenticate_headers(
    headers: &HeaderMap,
    auth: &AuthConfig,
) -> Option<annim::auth::AuthenticatedAdmin> {
    let authorization = headers
        .get("Authorization")
        .and_then(|value| value.to_str().ok())?;
    auth.authenticate_bearer(authorization).ok()
}

async fn graphql_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    req: GraphQLRequest,
) -> GraphQLResponse {
    let mut req = req.into_inner();
    if let Some(admin) = authenticate_headers(&headers, &state.auth) {
        req = req.data(admin);
    }
    state.schema.execute(req).await.into()
}

async fn graphql_ws_handler(
    State(state): State<AppState>,
    protocol: GraphQLProtocol,
    websocket: WebSocketUpgrade,
) -> Response {
    websocket
        .protocols(ALL_WEBSOCKET_PROTOCOLS)
        .on_upgrade(move |stream| {
            let auth = state.auth.clone();
            GraphQLWebSocket::new(stream, state.schema.clone(), protocol)
                .on_connection_init(move |value| on_connection_init(auth.clone(), value))
                .serve()
        })
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_test_writer()
        .init();

    let database_url = std::env::var("ANNIM_DATABASE_URL")?;
    let auth = AuthConfig::from_env()?;
    let database = Database::connect(database_url)
        .await
        .expect("Fail to initialize database connection");

    annim::migrator::Migrator::up(&database, None).await?;

    let searcher_directory = std::env::var("ANNIM_SEARCH_DIRECTORY")?;
    std::fs::create_dir_all(&searcher_directory)?;
    let searcher = RepositorySearchManager::open_or_create(searcher_directory)?;
    let schema = schema_builder(database).data(searcher).finish();

    let state = AppState { schema, auth };
    let app = Router::new()
        .route("/", get(graphql_playground).post(graphql_handler))
        .route("/ws", get(graphql_ws_handler))
        .layer(
            CorsLayer::new()
                .allow_methods([Method::GET, Method::POST])
                .allow_origin(cors::Any)
                .allow_headers(cors::Any),
        )
        .with_state(state);

    println!("Playground: http://localhost:8000");
    axum::serve(TcpListener::bind("0.0.0.0:8000").await?, app).await?;

    Ok(())
}
