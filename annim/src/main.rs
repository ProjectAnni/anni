use annim::{
    auth::{on_connection_init, AuthToken},
    graphql::{MetadataMutation, MetadataQuery, MetadataSchema},
};
use async_graphql::{
    http::{graphiql_source, ALL_WEBSOCKET_PROTOCOLS},
    EmptySubscription,
};
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
    response::Html(graphiql_source("/", None))
}

fn get_token_from_headers(headers: &HeaderMap) -> Option<AuthToken> {
    headers
        .get("Authorization")
        .and_then(|value| value.to_str().map(|s| AuthToken(s.to_string())).ok())
}

async fn graphql_handler(
    State(schema): State<MetadataSchema>,
    headers: HeaderMap,
    req: GraphQLRequest,
) -> GraphQLResponse {
    let mut req = req.into_inner();
    if let Some(token) = get_token_from_headers(&headers) {
        req = req.data(token);
    }
    schema.execute(req).await.into()
}

async fn graphql_ws_handler(
    State(schema): State<MetadataSchema>,
    protocol: GraphQLProtocol,
    websocket: WebSocketUpgrade,
) -> Response {
    websocket
        .protocols(ALL_WEBSOCKET_PROTOCOLS)
        .on_upgrade(move |stream| {
            GraphQLWebSocket::new(stream, schema.clone(), protocol)
                .on_connection_init(on_connection_init)
                .serve()
        })
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_test_writer()
        .init();

    let database = Database::connect("sqlite:///tmp/annim.sqlite?mode=rwc")
        .await
        .expect("Fail to initialize database connection");

    annim::migrator::Migrator::up(&database, None)
        .await
        .unwrap();

    let schema = MetadataSchema::build(MetadataQuery, MetadataMutation, EmptySubscription)
        .data(database)
        .finish();

    let app = Router::new()
        .route("/", get(graphql_playground).post(graphql_handler))
        .route("/ws", get(graphql_ws_handler))
        .layer(
            CorsLayer::new()
                .allow_methods([Method::GET, Method::POST])
                .allow_origin(cors::Any)
                .allow_headers(cors::Any),
        )
        .with_state(schema);

    println!("Playground: http://localhost:8000");

    axum::serve(TcpListener::bind("localhost:8000").await.unwrap(), app)
        .await
        .unwrap();
}
