use async_graphql::http::graphiql_source;
use async_graphql_axum::GraphQL;
use axum::{
    http::Method,
    response::{self, IntoResponse},
    routing::get,
    Router,
};
use dotenv::dotenv;
use lazy_static::lazy_static;
use sea_orm::Database;
use sea_orm_migration::MigratorTrait;
use std::env;
use tokio::net::TcpListener;
use tower_http::cors;
use tower_http::cors::CorsLayer;

lazy_static! {
    static ref URL: String = env::var("URL").unwrap_or("localhost:8000".into());
    static ref ENDPOINT: String = env::var("ENDPOINT").unwrap_or("/".into());
    static ref DATABASE_URL: String =
        env::var("DATABASE_URL").expect("DATABASE_URL environment variable not set");
    static ref DEPTH_LIMIT: Option<usize> = env::var("DEPTH_LIMIT").map_or(None, |data| Some(
        data.parse().expect("DEPTH_LIMIT is not a number")
    ));
    static ref COMPLEXITY_LIMIT: Option<usize> = env::var("COMPLEXITY_LIMIT")
        .map_or(None, |data| {
            Some(data.parse().expect("COMPLEXITY_LIMIT is not a number"))
        });
}

async fn graphql_playground() -> impl IntoResponse {
    response::Html(graphiql_source(&*ENDPOINT, None))
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_test_writer()
        .init();
    let database = Database::connect(&*DATABASE_URL)
        .await
        .expect("Fail to initialize database connection");

    annim::migrator::Migrator::up(&database, None)
        .await
        .unwrap();

    let schema = annim::model::build_schema(database);
    // let schema = annim::query_root::schema(database, *DEPTH_LIMIT, *COMPLEXITY_LIMIT).unwrap();
    let app = Router::new()
        .route(
            "/",
            get(graphql_playground).post_service(GraphQL::new(schema)),
        )
        .layer(
            CorsLayer::new()
                .allow_methods([Method::GET, Method::POST])
                .allow_origin(cors::Any)
                .allow_headers(cors::Any),
        );
    println!("Visit GraphQL Playground at http://{}", *URL);
    axum::serve(TcpListener::bind(&*URL).await.unwrap(), app)
        .await
        .unwrap();
}
