use std::{env, net::SocketAddr};

use anyhow::Context;

#[cfg(not(target_arch = "wasm32"))]
use tokio::net::TcpListener;
#[cfg(not(target_arch = "wasm32"))]
use tracing::info;

#[cfg(not(target_arch = "wasm32"))]
pub mod api;
#[cfg(not(target_arch = "wasm32"))]
pub mod app;
pub mod color_math;
pub mod demo;
#[cfg(not(target_arch = "wasm32"))]
pub mod messaging;
pub mod node_runtime;
mod platform_time;
pub mod services;

#[cfg(not(target_arch = "wasm32"))]
use api::http::{frontend_dist_dir, router};
#[cfg(not(target_arch = "wasm32"))]
use app::startup::build_app_state;

/// Starts the backend HTTP and WebSocket server.
#[cfg(not(target_arch = "wasm32"))]
pub async fn run_backend() -> anyhow::Result<()> {
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
pub fn backend_port() -> u16 {
    env::var("BACKEND_PORT")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(38123)
}
