/// Frontend application state and app-level actions.
mod app;
/// Browser-only helpers for graph import and export file flows.
mod browser_file;
/// Frontend controller logic for autosave and subscription synchronization.
mod controllers;
/// Dashboard screen rendering.
mod dashboard_view;
/// Shared graph diagnostics rendering used by dashboard and editor flows.
mod diagnostics_view;
/// Graph editor screen rendering and node-canvas integration.
mod editor_view;
/// Shared top application header rendering.
mod header_view;
/// Frontend state containers.
mod state;
#[cfg(target_arch = "wasm32")]
/// Browser WebSocket transport for the wasm frontend build.
mod websocket_client;

#[cfg(target_arch = "wasm32")]
use tracing::{error, info};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
/// Starts the wasm frontend and mounts the egui application into the browser canvas.
pub async fn start() -> Result<(), wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();
    info!("frontend wasm entrypoint started");

    let options = eframe::WebOptions::default();
    let web_runner = eframe::WebRunner::new();
    let canvas = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("app-canvas"))
        .and_then(|element| element.dyn_into::<web_sys::HtmlCanvasElement>().ok())
        .ok_or_else(|| {
            error!("frontend missing canvas element with id app-canvas");
            wasm_bindgen::JsValue::from_str("missing canvas element")
        })?;

    info!("frontend canvas located, starting web runner");

    web_runner
        .start(
            canvas,
            options,
            Box::new(|_cc| {
                let app = app::FrontendApp::default();
                info!("frontend application initialized");
                Ok(Box::new(app))
            }),
        )
        .await
}
