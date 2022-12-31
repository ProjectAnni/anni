use crate::subcommands::workspace::utils::find_dot_anni;
use anni_provider::providers::NoCacheStrictLocalProvider;
use annil::provider::AnnilProvider;
use annil::route::user;
use annil::state::{AnnilKeys, AnnilProviders, AnnilState};
use axum::routing::get;
use axum::{Extension, Router, Server};
use std::net::SocketAddr;
use std::sync::Arc;

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
use tokio::sync::RwLock;

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
    let root = find_dot_anni()?;
    let _repo_root = root.join("repo");
    let audio_root = root.join("objects");

    let annil_state = AnnilState {
        version: "0.0.1-SNAPSHOT".to_string(),
        last_update: Default::default(),
        etag: Default::default(),
        metadata: None,
    };
    let annil_providers = AnnilProviders(RwLock::new(vec![AnnilProvider::new(
        "".to_string(),
        Box::new(NoCacheStrictLocalProvider {
            root: audio_root,
            layer: 2,
        }),
    )]));
    let annil_keys = AnnilKeys::new("a token here".as_bytes(), "".as_bytes(), String::new());

    let annil = Router::new()
        .route("/info", get(user::info))
        .route("/albums", get(user::albums))
        .route(
            "/:album_id/:disc_id/:track_id",
            get(user::audio).head(user::audio_head),
        )
        .route("/cover/:album_id", get(user::cover))
        .route("/cover/:album_id/:disc_id", get(user::cover))
        .layer(Extension(Arc::new(annil_state)))
        .layer(Extension(Arc::new(annil_providers)))
        .layer(Extension(Arc::new(annil_keys)));

    let app = Router::new().nest("/l", annil);

    let addr = SocketAddr::from(([127, 0, 0, 1], this.port));
    Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();

    Ok(())
}
