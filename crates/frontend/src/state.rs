use std::collections::{HashMap, HashSet, VecDeque};

use futures_channel::mpsc;
use shared::{
    EventSubscription, GraphDocument, GraphExchangeFile, GraphMetadata, GraphRuntimeMode,
    InputValue, MqttBrokerConfig, NodeDefinition, NodeDiagnosticEntry, NodeDiagnosticSummary,
    ServerState, WledInstance,
};

use crate::transport::FrontendTransport;

#[derive(Clone, Copy, PartialEq, Eq)]
/// Identifies which top-level screen the frontend is currently rendering.
pub(crate) enum AppView {
    Dashboard,
    Editor,
}

/// Holds transient UI state that is not part of the persisted graph model.
pub(crate) struct UiState {
    pub(crate) status: String,
    pub(crate) last_event: String,
    pub(crate) active_view: AppView,
    pub(crate) selected_graph_id: Option<String>,
    pub(crate) create_graph_dialog_open: bool,
    pub(crate) new_graph_name: String,
    pub(crate) rename_graph_dialog_open: bool,
    pub(crate) rename_graph_id: Option<String>,
    pub(crate) rename_graph_name: String,
    pub(crate) step_tick_count: u32,
    pub(crate) diagnostics_window_graph_id: Option<String>,
    pub(crate) diagnostics_window_node_id: Option<String>,
    pub(crate) mqtt_broker_dialog_open: bool,
    pub(crate) mqtt_broker_draft: Vec<MqttBrokerConfig>,
    pub(crate) editor_canvas_hovered: bool,
    pub(crate) node_menu_search: String,
    pub(crate) node_menu_graph_position: Option<(f32, f32)>,
    pub(crate) pending_import_graph_file: Option<GraphExchangeFile>,
    #[cfg(target_arch = "wasm32")]
    pub(crate) browser_graph_file_events:
        Option<mpsc::UnboundedReceiver<crate::browser_file::BrowserGraphFileEvent>>,
}

impl Default for UiState {
    /// Builds the default transient UI state for a newly started frontend session.
    fn default() -> Self {
        Self {
            status: "Waiting for server messages".to_owned(),
            last_event: "No events received".to_owned(),
            active_view: AppView::Dashboard,
            selected_graph_id: None,
            create_graph_dialog_open: false,
            new_graph_name: String::new(),
            rename_graph_dialog_open: false,
            rename_graph_id: None,
            rename_graph_name: String::new(),
            step_tick_count: 1,
            diagnostics_window_graph_id: None,
            diagnostics_window_node_id: None,
            mqtt_broker_dialog_open: false,
            mqtt_broker_draft: Vec::new(),
            editor_canvas_hovered: false,
            node_menu_search: String::new(),
            node_menu_graph_position: None,
            pending_import_graph_file: None,
            #[cfg(target_arch = "wasm32")]
            browser_graph_file_events: None,
        }
    }
}

/// Holds graph-centric frontend state such as metadata, the open document, runtime previews, and history.
pub(crate) struct GraphState {
    pub(crate) graph_documents: Vec<GraphMetadata>,
    pub(crate) available_node_definitions: Vec<NodeDefinition>,
    pub(crate) loaded_graph_document: Option<GraphDocument>,
    pub(crate) history_committed_document: Option<GraphDocument>,
    pub(crate) undo_history: Vec<GraphDocument>,
    pub(crate) redo_history: Vec<GraphDocument>,
    pub(crate) requested_graph_document_id: Option<String>,
    pub(crate) persisted_graph_document: Option<GraphDocument>,
    pub(crate) save_in_flight_document: Option<GraphDocument>,
    pub(crate) pending_graph_update: bool,
    pub(crate) graph_update_dirty_since_secs: Option<f64>,
    pub(crate) graph_update_last_change_secs: Option<f64>,
    pub(crate) graph_update_last_observed_document: Option<GraphDocument>,
    pub(crate) snarl_viewport_initialized_graph_id: Option<String>,
    pub(crate) graph_runtime_modes: HashMap<String, GraphRuntimeMode>,
    pub(crate) runtime_node_values: HashMap<String, HashMap<String, InputValue>>,
    pub(crate) plot_history: HashMap<String, VecDeque<f32>>,
    pub(crate) node_diagnostic_summaries_by_graph:
        HashMap<String, HashMap<String, NodeDiagnosticSummary>>,
    pub(crate) node_diagnostic_details_by_graph:
        HashMap<String, HashMap<String, Vec<NodeDiagnosticEntry>>>,
    pub(crate) wled_instances: Vec<WledInstance>,
    pub(crate) mqtt_broker_configs: Vec<MqttBrokerConfig>,
}

impl Default for GraphState {
    /// Builds the empty graph state used before backend data has been loaded.
    fn default() -> Self {
        Self {
            graph_documents: Vec::new(),
            available_node_definitions: Vec::new(),
            loaded_graph_document: None,
            history_committed_document: None,
            undo_history: Vec::new(),
            redo_history: Vec::new(),
            requested_graph_document_id: None,
            persisted_graph_document: None,
            save_in_flight_document: None,
            pending_graph_update: false,
            graph_update_dirty_since_secs: None,
            graph_update_last_change_secs: None,
            graph_update_last_observed_document: None,
            snarl_viewport_initialized_graph_id: None,
            graph_runtime_modes: HashMap::new(),
            runtime_node_values: HashMap::new(),
            plot_history: HashMap::new(),
            node_diagnostic_summaries_by_graph: HashMap::new(),
            node_diagnostic_details_by_graph: HashMap::new(),
            wled_instances: Vec::new(),
            mqtt_broker_configs: Vec::new(),
        }
    }
}

/// Tracks which backend event streams and one-shot requests the frontend currently wants active.
pub(crate) struct SubscriptionState {
    pub(crate) subscribe_connection: bool,
    pub(crate) subscribe_ping: bool,
    pub(crate) subscribe_name: bool,
    pub(crate) subscribe_graph_metadata_changed: bool,
    pub(crate) active_event_subscriptions: HashSet<EventSubscription>,
    pub(crate) subscriptions_status: String,
    pub(crate) initialized: bool,
    pub(crate) metadata_requested_once: bool,
    pub(crate) node_definitions_requested_once: bool,
    pub(crate) running_graphs_requested_once: bool,
    pub(crate) runtime_graph_subscription: Option<String>,
    pub(crate) diagnostics_graph_subscriptions: HashSet<String>,
    pub(crate) diagnostics_node_subscription: Option<(String, String)>,
    pub(crate) wled_instances_requested_once: bool,
    pub(crate) mqtt_brokers_requested_once: bool,
}

impl Default for SubscriptionState {
    /// Enables the default global event subscriptions and clears all derived subscription state.
    fn default() -> Self {
        Self {
            subscribe_connection: true,
            subscribe_ping: true,
            subscribe_name: true,
            subscribe_graph_metadata_changed: true,
            active_event_subscriptions: HashSet::new(),
            subscriptions_status: "No active subscriptions".to_owned(),
            initialized: false,
            metadata_requested_once: false,
            node_definitions_requested_once: false,
            running_graphs_requested_once: false,
            runtime_graph_subscription: None,
            diagnostics_graph_subscriptions: HashSet::new(),
            diagnostics_node_subscription: None,
            wled_instances_requested_once: false,
            mqtt_brokers_requested_once: false,
        }
    }
}

/// Holds the current frontend transport state and reconnect bookkeeping.
pub(crate) struct ConnectionState {
    pub(crate) server_state: ServerState,
    pub(crate) ws_status: String,
    pub(crate) has_confirmed_connection: bool,
    pub(crate) transport: Option<FrontendTransport>,
    pub(crate) reconnect_attempt: u32,
    pub(crate) next_reconnect_at_secs: f64,
}

impl Default for ConnectionState {
    /// Builds the disconnected initial connection state.
    fn default() -> Self {
        Self {
            server_state: ServerState::default(),
            ws_status: "Disconnected".to_owned(),
            has_confirmed_connection: false,
            transport: None,
            reconnect_attempt: 0,
            next_reconnect_at_secs: 0.0,
        }
    }
}

impl ConnectionState {
    /// Clears all live WebSocket channels and resets the confirmed-connection flag.
    pub(crate) fn clear_channels(&mut self) {
        self.transport = None;
        self.has_confirmed_connection = false;
    }

    /// Returns whether a transport is currently connected or in the process of connecting.
    pub(crate) fn is_connected(&self) -> bool {
        self.transport.is_some()
    }

    /// Schedules the next reconnect attempt using exponential backoff capped by attempt count.
    pub(crate) fn schedule_reconnect(&mut self, now_secs: f64) {
        let delay_secs = 2f64.powi(self.reconnect_attempt.min(4) as i32);
        self.next_reconnect_at_secs = now_secs + delay_secs;
        self.reconnect_attempt = self.reconnect_attempt.saturating_add(1);
    }
}
