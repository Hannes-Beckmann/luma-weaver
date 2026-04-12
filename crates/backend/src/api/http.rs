use std::path::PathBuf;

use axum::{
    Json, Router,
    body::Bytes,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use serde::Serialize;
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
        .route("/api/assets/images", post(upload_image_asset))
        .fallback_service(
            ServeDir::new(dist_dir)
                .append_index_html_on_directories(true)
                .not_found_service(ServeFile::new(index_file)),
        )
        .with_state(state)
        .layer(TraceLayer::new_for_http())
}

#[derive(Debug, Serialize)]
struct UploadImageAssetResponse {
    asset_id: String,
}

/// Persists an uploaded image asset and returns the stable id the graph should reference.
async fn upload_image_asset(
    State(state): State<AppState>,
    body: Bytes,
) -> Result<Json<UploadImageAssetResponse>, (StatusCode, String)> {
    let asset_id = state
        .image_asset_store
        .store_image_bytes(body.as_ref())
        .map_err(invalid_upload_response)?;
    Ok(Json(UploadImageAssetResponse { asset_id }))
}

fn invalid_upload_response(error: anyhow::Error) -> (StatusCode, String) {
    let message = error.to_string();
    let status = if message.contains("decode uploaded image") || message.contains("empty") {
        StatusCode::BAD_REQUEST
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    };
    (status, message)
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
