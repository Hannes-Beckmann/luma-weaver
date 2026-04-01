use std::path::PathBuf;

use axum::{Router, routing::get};
use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};

use crate::api::websocket::websocket_handler;
use crate::app::state::AppState;

/// Builds the Axum router for the backend HTTP server.
///
/// The router serves a health endpoint, the backend WebSocket endpoint, and the compiled frontend
/// assets as a static fallback.
pub(crate) fn router(state: AppState) -> Router {
    let dist_dir = frontend_dist_dir();
    let index_file = dist_dir.join("index.html");

    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/ws", get(websocket_handler))
        .fallback_service(
            ServeDir::new(dist_dir)
                .append_index_html_on_directories(true)
                .not_found_service(ServeFile::new(index_file)),
        )
        .with_state(state)
        .layer(TraceLayer::new_for_http())
}

/// Returns the filesystem path containing the built frontend assets served by the backend.
pub(crate) fn frontend_dist_dir() -> PathBuf {
    std::env::var("FRONTEND_DIST_DIR")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../frontend/dist"))
}
