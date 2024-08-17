mod migrator;
mod model;

use anni_repo::RepositoryManager;
use async_graphql::http::{playground_source, GraphQLPlaygroundConfig};
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::{
    response::{Html, IntoResponse},
    routing::get,
    Extension, Router,
};
use model::{build_schema, AppSchema};
use sea_orm::{ConnectionTrait, Database, DbBackend};
use std::env::args;
use tokio::net::TcpListener;

async fn graphql_handler(schema: Extension<AppSchema>, req: GraphQLRequest) -> GraphQLResponse {
    schema.execute(req.into_inner()).await.into()
}

async fn graphql_playground() -> impl IntoResponse {
    Html(playground_source(GraphQLPlaygroundConfig::new("/graphql")))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let db = Database::connect("sqlite://tmp/annim.sqlite?mode=rwc").await?;
    let db = &match db.get_database_backend() {
        DbBackend::MySql | DbBackend::Postgres => {
            unimplemented!()
        }
        DbBackend::Sqlite => db,
    };

    Ok(())
}
