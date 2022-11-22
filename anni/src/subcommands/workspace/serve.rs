use super::utils::find_workspace_root;

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
    let root = find_workspace_root()?;
    let _repo_root = root.join("repo");
    todo!()
}
