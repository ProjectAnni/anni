use anni_provider::providers::NoCacheStrictLocalProvider;
use anni_workspace::AnniWorkspace;
use annil::provider::AnnilProvider;
use annil::route::user;
use annil::state::{AnnilKeys, AnnilState};
use axum::routing::get;
use axum::{Extension, Router};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;

/// The `serve` subcommand would launch three web services:
///
/// 1. An `annil` server to serve music and cover.
/// 2. A GraphQL API server to serve the metadata.
/// 3. [Optional] A WebSocket based server to redirect terminal interactions.
///
/// The server reuses the same port and use different endpoints for services.
/// All service endpoints are customizable.
use clap::Args;
use clap_handler::handler;

#[derive(Args, Debug, Clone)]
pub struct WorkspaceServeAction {
    // Let's use a delicious duck as default port
    #[clap(long, default_value = "6655")]
    pub port: u16,
    #[clap(long = "annil", default_value = "/l")]
    pub annil_endpoint: String,
    #[clap(long = "metadata", default_value = "/metadata")]
    pub metadata_endpoint: String,
    #[clap(long = "ws", default_value = "/ws")]
    pub websocket_endpoint: String,
}

#[handler(WorkspaceServeAction)]
pub async fn handle_workspace_serve(this: WorkspaceServeAction) -> anyhow::Result<()> {
    let workspace = AnniWorkspace::new()?;
    let _repo_root = workspace.repo_root();
    let audio_root = workspace.objects_root();

    let annil_state = AnnilState {
        version: "0.0.1-SNAPSHOT".to_string(),
        last_update: Default::default(),
        etag: Default::default(),
        metadata: None,
    };
    let annil_provider = AnnilProvider::new(NoCacheStrictLocalProvider {
        root: audio_root,
        layer: 2,
    });
    let annil_keys = AnnilKeys::new("a token here".as_bytes(), "".as_bytes(), String::new());

    type Provider = NoCacheStrictLocalProvider;
    let annil = Router::new()
        .route("/info", get(user::info))
        .route("/albums", get(user::albums::<Provider>))
        .route(
            "/:album_id/:disc_id/:track_id",
            get(user::audio::<Provider>).head(user::audio_head::<Provider>),
        )
        .route("/:album_id/cover", get(user::cover::<Provider>))
        .route("/:album_id/:disc_id/cover", get(user::cover::<Provider>))
        .layer(Extension(Arc::new(annil_state)))
        .layer(Extension(Arc::new(annil_provider)))
        .layer(Extension(Arc::new(annil_keys)));

    let app = Router::new().nest("/l", annil);

    let addr = SocketAddr::from(([127, 0, 0, 1], this.port));
    let listener = TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())
}
