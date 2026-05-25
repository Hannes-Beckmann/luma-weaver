use std::path::PathBuf;

use axum::{
    Json, Router,
    body::Bytes,
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post},
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
        .route("/api/assets/layouts", post(upload_layout_asset))
        .route("/api/assets/layouts/{asset_id}", delete(delete_layout_asset))
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

#[derive(Debug, Serialize)]
struct UploadLayoutAssetResponse {
    asset_id: String,
}

#[derive(Clone, Copy)]
enum UploadKind {
    Image,
    Layout,
}

/// Persists an uploaded image asset and returns the stable id the graph should reference.
async fn upload_image_asset(
    State(state): State<AppState>,
    body: Bytes,
) -> Result<Json<UploadImageAssetResponse>, (StatusCode, String)> {
    let asset_id = state
        .image_asset_store
        .store_image_bytes(body.as_ref())
        .map_err(|error| invalid_upload_response(error, UploadKind::Image))?;
    Ok(Json(UploadImageAssetResponse { asset_id }))
}

/// Persists an uploaded layout asset and returns the stable id the graph should reference.
async fn upload_layout_asset(
    State(state): State<AppState>,
    body: Bytes,
) -> Result<Json<UploadLayoutAssetResponse>, (StatusCode, String)> {
    let asset_id = state
        .layout_asset_store
        .store_layout_bytes(body.as_ref())
        .map_err(|error| invalid_upload_response(error, UploadKind::Layout))?;
    Ok(Json(UploadLayoutAssetResponse { asset_id }))
}

/// Deletes a previously uploaded layout asset.
async fn delete_layout_asset(
    State(state): State<AppState>,
    Path(asset_id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    state
        .layout_asset_store
        .delete_layout_asset(asset_id.trim())
        .map_err(invalid_layout_delete_response)?;
    Ok(StatusCode::NO_CONTENT)
}

fn invalid_upload_response(error: anyhow::Error, kind: UploadKind) -> (StatusCode, String) {
    let message = error.to_string();
    let is_bad_request = match kind {
        UploadKind::Image => {
            message.contains("decode uploaded image")
                || message.contains("uploaded image is empty")
        }
        UploadKind::Layout => {
            message.contains("uploaded layout")
                || message.contains("parse uploaded layout")
                || message.contains("decode uploaded layout as UTF-8")
        }
    };
    let status = if is_bad_request {
        StatusCode::BAD_REQUEST
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    };
    (status, message)
}

fn invalid_layout_delete_response(error: anyhow::Error) -> (StatusCode, String) {
    let message = error.to_string();
    let status = if message.contains("layout asset id")
        && message.contains("is invalid")
    {
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
