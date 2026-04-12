mod clipboard;
mod graph_actions;
mod history;
mod image_assets;
mod import_export;
mod messaging;
mod navigation;
mod server_state;

use std::time::Duration;

use eframe::egui;
use shared::GraphDocument;

use crate::state::{AppView, ConnectionState, GraphState, SubscriptionState, UiState};

pub struct FrontendApp {
    pub(crate) ui: UiState,
    pub(crate) graphs: GraphState,
    pub(crate) subscriptions: SubscriptionState,
    pub(crate) connection: ConnectionState,
}

pub(super) const MAX_UNDO_HISTORY: usize = 100;

impl Default for FrontendApp {
    /// Creates the frontend app state and seeds it from the current browser location.
    fn default() -> Self {
        let mut app = Self {
            ui: UiState::default(),
            graphs: GraphState::default(),
            subscriptions: SubscriptionState::default(),
            connection: ConnectionState::default(),
        };
        app.initialize_from_browser_path();
        app
    }
}

impl eframe::App for FrontendApp {
    /// Advances one egui frame by syncing transport state, rendering the active view, and
    /// scheduling any deferred graph persistence work.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.sync_state_from_browser_path();
        self.maintain_connection(ctx);
        self.pump_connection(ctx);
        self.sync_event_subscriptions_and_request_initial_data();
        self.ensure_runtime_updates_subscription();
        self.drain_server_messages(ctx);
        self.drain_browser_graph_file_events();
        self.drain_browser_clipboard_events();
        self.drain_browser_image_asset_events();
        crate::header_view::render(ctx, self);

        egui::CentralPanel::default().show(ctx, |ui| match self.ui.active_view {
            AppView::Dashboard => crate::dashboard_view::render(ctx, ui, self),
            AppView::Editor => crate::editor_view::render(ui, self),
        });

        self.handle_history_shortcuts(ctx);
        self.schedule_graph_document_update(ctx);

        if self.demo_runtime_needs_repaint() || self.browser_background_work_needs_repaint() {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
    }
}

impl FrontendApp {
    /// Rebuilds the live editor snarl from the currently loaded document.
    pub(crate) fn rebuild_live_snarl_from_loaded_document(&mut self) {
        let Some(document) = self.graphs.loaded_graph_document.clone() else {
            self.graphs.live_snarl_graph_id = None;
            self.graphs.live_snarl = None;
            self.graphs.live_snarl_needs_rebuild = false;
            return;
        };

        self.graphs.live_snarl_graph_id = Some(document.metadata.id.clone());
        self.graphs.live_snarl = Some(crate::editor_view::build_snarl_from_document(
            &document,
            &self.graphs.available_node_definitions,
            &self.graphs.runtime_node_values,
        ));
        self.graphs.live_snarl_needs_rebuild = false;
    }

    /// Synchronizes the live snarl with the currently loaded document, patching in place when
    /// possible so surviving nodes keep their `egui_snarl` identities across history restores.
    pub(crate) fn sync_live_snarl_from_loaded_document(&mut self) {
        let Some(document) = self.graphs.loaded_graph_document.clone() else {
            self.graphs.live_snarl_graph_id = None;
            self.graphs.live_snarl = None;
            self.graphs.live_snarl_needs_rebuild = false;
            return;
        };

        let loaded_graph_id = document.metadata.id.clone();
        let can_patch = self.graphs.live_snarl_graph_id.as_deref()
            == Some(loaded_graph_id.as_str())
            && self.graphs.live_snarl.is_some();

        if can_patch {
            if let Some(snarl) = self.graphs.live_snarl.as_mut() {
                crate::editor_view::patch_snarl_from_document(
                    snarl,
                    &document,
                    &self.graphs.available_node_definitions,
                    &self.graphs.runtime_node_values,
                );
            }
            self.graphs.live_snarl_needs_rebuild = false;
            return;
        }

        self.rebuild_live_snarl_from_loaded_document();
    }

    /// Ensures the selected graph has a live snarl instance ready for rendering.
    pub(crate) fn ensure_live_snarl_for_active_graph(&mut self) {
        let Some(selected_graph_id) = self.ui.selected_graph_id.as_deref() else {
            return;
        };
        let loaded_graph_id = self
            .graphs
            .loaded_graph_document
            .as_ref()
            .map(|document| document.metadata.id.as_str());
        if loaded_graph_id != Some(selected_graph_id) {
            return;
        }

        let live_graph_matches =
            self.graphs.live_snarl_graph_id.as_deref() == Some(selected_graph_id);
        if self.graphs.live_snarl.is_none()
            || !live_graph_matches
            || self.graphs.live_snarl_needs_rebuild
        {
            self.sync_live_snarl_from_loaded_document();
        }
    }

    /// Restores a graph-history snapshot while preserving the current editor viewport.
    ///
    /// Undo and redo intentionally treat camera movement as transient UI state, so history
    /// snapshots restore graph content without rewinding the current pan and zoom position.
    pub(super) fn restore_graph_history_snapshot(&mut self, mut document: GraphDocument) {
        if let Some(current) = self.graphs.loaded_graph_document.as_ref() {
            document.viewport = current.viewport.clone();
        }
        self.graphs.loaded_graph_document = Some(document.clone());
        self.graphs.history_committed_document = Some(document.clone());
        self.graphs.save_in_flight_document = None;
        self.graphs.graph_update_last_observed_document = Some(document);
        self.graphs.live_snarl_needs_rebuild = false;
        self.sync_live_snarl_from_loaded_document();
        self.clear_pending_graph_update_tracking();
    }
}

#[cfg(test)]
mod tests {
    use super::FrontendApp;
    use crate::state::AppView;

    #[test]
    fn default_app_starts_disconnected_on_dashboard() {
        let app = FrontendApp::default();

        assert!(matches!(app.ui.active_view, AppView::Dashboard));
        assert!(app.ui.selected_graph_id.is_none());
        assert!(app.graphs.graph_documents.is_empty());
        assert_eq!(app.connection.ws_status, "Disconnected");
        assert!(!app.connection.has_confirmed_connection);
    }
}
