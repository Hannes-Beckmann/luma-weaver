mod graph_actions;
mod history;
mod import_export;
mod messaging;
mod navigation;
mod server_state;

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
        self.drain_websocket_events(ctx);
        self.sync_event_subscriptions_and_request_initial_data();
        self.ensure_runtime_updates_subscription();
        self.drain_server_messages(ctx);
        self.drain_browser_graph_file_events();
        crate::header_view::render(ctx, self);

        egui::CentralPanel::default().show(ctx, |ui| match self.ui.active_view {
            AppView::Dashboard => crate::dashboard_view::render(ctx, ui, self),
            AppView::Editor => crate::editor_view::render(ui, self),
        });

        self.handle_history_shortcuts(ctx);
        self.schedule_graph_document_update(ctx);
    }
}

impl FrontendApp {
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
        self.clear_pending_graph_update_tracking();
    }
}
