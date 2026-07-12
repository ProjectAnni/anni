use annim::{
    auth::AuthConfig,
    config::ServerConfig,
    graphql::schema_builder,
    search::RepositorySearchManager,
    server::{build_router, ServerState},
};
use sea_orm::Database;
use sea_orm_migration::MigratorTrait;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_test_writer()
        .init();

    let database_url = std::env::var("ANNIM_DATABASE_URL")?;
    let auth = AuthConfig::from_env()?;
    let server_config = ServerConfig::from_env()?;
    let database = Database::connect(database_url).await?;

    annim::migrator::Migrator::up(&database, None).await?;

    let searcher_directory = std::env::var("ANNIM_SEARCH_DIRECTORY")?;
    std::fs::create_dir_all(&searcher_directory)?;
    let searcher = RepositorySearchManager::open_or_create(searcher_directory)?;
    let schema = schema_builder(database.clone()).data(searcher).finish();
    let state = ServerState::new(schema, auth, database);
    let app = build_router(state, &server_config);

    let bind_addr = server_config.bind_addr();
    tracing::info!(%bind_addr, "Annim server listening");
    if !bind_addr.ip().is_loopback() {
        tracing::warn!("non-loopback listeners should be protected by a TLS reverse proxy");
    }
    axum::serve(TcpListener::bind(bind_addr).await?, app).await?;

    Ok(())
}
