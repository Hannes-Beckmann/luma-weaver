use std::{env, net::SocketAddr};

use anyhow::Context;
use tokio::net::TcpListener;
use tracing::info;

mod api;
mod app;
mod color_math;
mod messaging;
mod node_runtime;
mod services;

use api::http::{frontend_dist_dir, router};
use app::startup::build_app_state;

/// Starts the backend HTTP and WebSocket server.
///
/// Startup initializes tracing, builds shared application state, binds the listening socket, and
/// then serves the Axum router until shutdown.
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let state = build_app_state().await?;
    info!(frontend_dist = %frontend_dist_dir().display(), "starting backend");
    let app = router(state);

    let port = backend_port();
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr)
        .await
        .context("bind backend listener")?;

    info!("backend listening on http://{addr}");
    axum::serve(listener, app).await.context("serve backend")?;
    Ok(())
}

/// Returns the backend listener port from `BACKEND_PORT`, falling back to the production default.
fn backend_port() -> u16 {
    env::var("BACKEND_PORT")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(38123)
}
