use std::collections::HashSet;

use shared::{
    GraphDocument, GraphMetadata, GraphRuntimeMode, GraphRuntimeStatus, InputValue,
    MqttBrokerConfig, NodeDiagnosticEntry, NodeDiagnosticSummary, NodeRuntimeUpdateValue,
    NodeSchema, ServerState, SinkPreviewFrame, WledInstance,
};
use tracing::{debug, info, trace, warn};

use super::FrontendApp;
use crate::state::AppView;

impl FrontendApp {
    /// Records a newly established WebSocket connection and resets client-side sync state.
    ///
    /// This stores the fresh transport channels, clears any reconnect backoff, and marks all
    /// derived subscription state as needing to be rebuilt against the new connection.
    pub(crate) fn handle_connected(
        &mut self,
        ws_status: String,
        transport: crate::transport::FrontendTransport,
        repaint_ctx: eframe::egui::Context,
    ) {
        self.connection.ws_status = ws_status;
        self.connection.has_confirmed_connection = false;
        self.connection.transport = Some(transport);
        self.connection.repaint_ctx = Some(repaint_ctx);
        self.connection.reconnect_attempt = 0;
        self.reset_subscription_sync_state();
        info!("frontend connection established");
        self.ui.status = "Connecting to backend".to_owned();
    }

    /// Tears down the current WebSocket session and schedules the next reconnect attempt.
    ///
    /// Any in-flight graph save is abandoned so the autosave path can retry once the connection
    /// is restored.
    pub(crate) fn handle_disconnected(&mut self, now_secs: f64, ws_status: String) {
        self.connection.ws_status = ws_status;
        self.connection.clear_channels();
        self.connection.schedule_reconnect(now_secs);
        self.reset_subscription_sync_state();
        self.graphs.save_in_flight_document = None;
        warn!("frontend connection lost; reconnect scheduled");
        self.ui.status = "Connection lost, reconnecting".to_owned();
    }

    /// Replaces the last server heartbeat snapshot shown in the UI.
    pub(crate) fn apply_server_state(&mut self, server_state: ServerState) {
        self.connection.server_state = server_state;
    }

    /// Replaces the dashboard metadata list and drops runtime state for graphs that no longer exist.
    ///
    /// If the currently open editor graph disappears, the editor session is cleared and the UI
    /// returns to the dashboard.
    pub(crate) fn apply_graph_metadata(&mut self, documents: Vec<GraphMetadata>) {
        self.graphs.graph_documents = documents;
        let known_graph_ids = self
            .graphs
            .graph_documents
            .iter()
            .map(|graph| graph.id.as_str())
            .collect::<HashSet<_>>();
        self.graphs
            .graph_runtime_modes
            .retain(|graph_id, _| known_graph_ids.contains(graph_id.as_str()));
        self.graphs
            .node_diagnostic_summaries_by_graph
            .retain(|graph_id, _| known_graph_ids.contains(graph_id.as_str()));
        self.graphs
            .node_diagnostic_details_by_graph
            .retain(|graph_id, _| known_graph_ids.contains(graph_id.as_str()));
        self.graphs
            .preview_frames_by_graph
            .retain(|graph_id, _| known_graph_ids.contains(graph_id.as_str()));
        self.ui.status = format!(
            "Loaded {} graph documents",
            self.graphs.graph_documents.len()
        );

        if self.selected_graph().is_none() && self.ui.active_view == AppView::Editor {
            self.ui.status = "Selected graph was removed".to_owned();
            self.clear_selected_graph_session();
            self.ui.active_view = AppView::Dashboard;
            self.sync_browser_path_for_current_view();
        }
    }

    /// Loads a graph document into the editor and resets local state derived from the previous graph.
    ///
    /// When the loaded graph changes, runtime previews, plot history, and viewport initialization
    /// are cleared so the editor starts from a clean session for that document.
    pub(crate) fn apply_graph_document_loaded(&mut self, document: GraphDocument) {
        let graph_id = document.metadata.id.clone();
        let same_loaded_graph = self
            .graphs
            .loaded_graph_document
            .as_ref()
            .map(|loaded| loaded.metadata.id.as_str())
            == Some(graph_id.as_str());

        self.graphs.requested_graph_document_id = Some(document.metadata.id.clone());
        self.graphs.persisted_graph_document = Some(document.clone());
        self.graphs.loaded_graph_document = Some(document.clone());
        self.reset_graph_history(Some(document));
        self.graphs.save_in_flight_document = None;

        if !same_loaded_graph {
            self.graphs.snarl_viewport_initialized_graph_id = None;
            self.graphs.runtime_node_values.clear();
            self.graphs.preview_frames_by_graph.remove(&graph_id);
            self.graphs.plot_history.clear();
        }

        self.sync_live_snarl_from_loaded_document();
        self.clear_pending_graph_update_tracking();

        info!(graph_id = %graph_id, "frontend loaded graph document");
        self.ui.status = "Graph document loaded".to_owned();
    }

    /// Applies a save acknowledgement from the backend to the persisted-document baseline.
    ///
    /// If the acknowledged document still matches the current editor contents, the graph becomes
    /// clean. Otherwise the acknowledgement only advances the persisted baseline and leaves newer
    /// local edits dirty.
    pub(crate) fn acknowledge_graph_save(
        &mut self,
        document: GraphDocument,
        current_document_matches_ack: bool,
    ) {
        let graph_id = document.metadata.id.clone();
        self.graphs.requested_graph_document_id = Some(document.metadata.id.clone());
        self.graphs.persisted_graph_document = Some(document.clone());
        self.graphs.save_in_flight_document = None;

        if current_document_matches_ack {
            self.graphs.loaded_graph_document = Some(document.clone());
            self.graphs.history_committed_document = Some(document);
            self.sync_live_snarl_from_loaded_document();
            self.clear_pending_graph_update_tracking();
            info!(graph_id = %graph_id, "frontend save acknowledged");
            self.ui.status = "Graph document saved".to_owned();
        } else {
            debug!(graph_id = %graph_id, "frontend save acknowledged while newer local changes remain");
            self.ui.status = "Graph saved, newer local changes are still pending".to_owned();
        }
    }

    /// Replaces the set of node definitions available to the editor palette.
    pub(crate) fn apply_node_definitions(&mut self, definitions: Vec<NodeSchema>) {
        self.graphs.available_node_definitions = definitions;
        self.graphs.live_snarl_needs_rebuild = false;
        self.sync_live_snarl_from_loaded_document();
        self.ui.status = "Node definitions updated".to_owned();
    }

    /// Marks the current graph as still dirty after a save attempt failed.
    ///
    /// This re-arms the autosave debounce timers so the document can be retried once connectivity
    /// or backend availability recovers.
    pub(crate) fn mark_graph_save_failed(&mut self, now_secs: f64) {
        if self.graphs.save_in_flight_document.take().is_some() {
            self.graphs.pending_graph_update = true;
            self.graphs
                .graph_update_dirty_since_secs
                .get_or_insert(now_secs);
            self.graphs.graph_update_last_change_secs = Some(now_secs);
            warn!("frontend save failed; graph remains dirty");
        }
    }

    /// Replaces the cached runtime mode for every known graph.
    pub(crate) fn apply_runtime_statuses(&mut self, graphs: Vec<GraphRuntimeStatus>) {
        let selected_graph_id = self.ui.selected_graph_id.clone();
        let previous_selected_mode = selected_graph_id
            .as_ref()
            .and_then(|graph_id| self.graphs.graph_runtime_modes.get(graph_id).copied());
        self.graphs.graph_runtime_modes = graphs
            .into_iter()
            .map(|status| (status.graph_id, status.mode))
            .collect();
        if let Some(graph_id) = selected_graph_id {
            let next_selected_mode = self.graphs.graph_runtime_modes.get(&graph_id).copied();
            let started_running = matches!(
                (previous_selected_mode, next_selected_mode),
                (None, Some(GraphRuntimeMode::Running))
                    | (
                        Some(GraphRuntimeMode::Paused),
                        Some(GraphRuntimeMode::Running)
                    )
            );
            if started_running {
                self.graphs.runtime_node_values.clear();
                self.graphs.plot_history.clear();
                self.graphs.preview_frames_by_graph.remove(&graph_id);
            }
        }
        self.ui.status = "Runtime statuses updated".to_owned();
    }

    /// Applies a runtime update to the currently open graph.
    ///
    /// Updates for other graphs are ignored. Plot nodes additionally append their `value` samples
    /// into bounded history so the editor can render a scrolling plot preview.
    pub(crate) fn apply_runtime_update(
        &mut self,
        graph_id: String,
        node_id: String,
        values: Vec<NodeRuntimeUpdateValue>,
    ) {
        let loaded_graph_id = self
            .graphs
            .loaded_graph_document
            .as_ref()
            .map(|document| document.metadata.id.as_str());
        if loaded_graph_id != Some(graph_id.as_str()) {
            return;
        }

        trace!(graph_id = %graph_id, node_id = %node_id, value_count = values.len(), "frontend applying runtime update");
        let is_plot_node = self
            .graphs
            .loaded_graph_document
            .as_ref()
            .and_then(|document| document.nodes.iter().find(|node| node.id == node_id))
            .map(|node| node.node_type.as_str() == shared::NodeTypeId::PLOT)
            .unwrap_or(false);
        let node_values = self
            .graphs
            .runtime_node_values
            .entry(node_id.clone())
            .or_default();
        for value in values {
            let (name, value) = decode_runtime_update_value(value);
            if is_plot_node && name == "value" {
                if let InputValue::Float(sample) = value {
                    let history = self.graphs.plot_history.entry(node_id.clone()).or_default();
                    history.push_back(sample);
                    while history.len() > 256 {
                        history.pop_front();
                    }
                    node_values.insert(name, InputValue::Float(sample));
                    continue;
                }
            }
            node_values.insert(name, value);
        }
        self.refresh_graph_preview_frames(&graph_id);
    }

    /// Replaces the node-level diagnostic summary list for the selected graph.
    pub(crate) fn apply_graph_diagnostics_summary(
        &mut self,
        graph_id: String,
        nodes: Vec<NodeDiagnosticSummary>,
    ) {
        let summaries = nodes
            .into_iter()
            .map(|summary| (summary.node_id.clone(), summary))
            .collect();
        self.graphs
            .node_diagnostic_summaries_by_graph
            .insert(graph_id, summaries);
    }

    /// Stores the full diagnostic entries for a single node in the selected graph.
    pub(crate) fn apply_node_diagnostics_detail(
        &mut self,
        graph_id: String,
        node_id: String,
        diagnostics: Vec<NodeDiagnosticEntry>,
    ) {
        self.graphs
            .node_diagnostic_details_by_graph
            .entry(graph_id)
            .or_default()
            .insert(node_id, diagnostics);
    }

    /// Replaces the discovered WLED instance list shown in the editor.
    pub(crate) fn apply_wled_instances(&mut self, instances: Vec<WledInstance>) {
        self.graphs.wled_instances = instances;
    }

    /// Replaces the configured MQTT broker list available to Home Assistant MQTT nodes.
    pub(crate) fn apply_mqtt_broker_configs(&mut self, brokers: Vec<MqttBrokerConfig>) {
        self.graphs.mqtt_broker_configs = brokers;
        self.ui.status = "MQTT broker configs updated".to_owned();
    }

    /// Clears all editor state tied to the currently selected graph session.
    ///
    /// This resets transient UI such as menus, rename dialogs, runtime previews, the open
    /// diagnostics window, and undo history in addition to dropping the loaded document itself.
    pub(crate) fn clear_selected_graph_session(&mut self) {
        self.ui.selected_graph_id = None;
        self.ui.editor_canvas_hovered = false;
        self.ui.selected_graph_node_ids.clear();
        self.ui.editor_pointer_graph_position = None;
        self.ui.pending_clipboard_read_graph_id = None;
        self.ui.node_menu_search.clear();
        self.ui.node_menu_graph_position = None;
        self.ui.rename_graph_dialog_open = false;
        self.ui.rename_graph_id = None;
        self.ui.rename_graph_name.clear();
        self.graphs.loaded_graph_document = None;
        self.reset_graph_history(None);
        self.graphs.requested_graph_document_id = None;
        self.graphs.persisted_graph_document = None;
        self.graphs.save_in_flight_document = None;
        self.clear_pending_graph_update_tracking();
        self.graphs.snarl_viewport_initialized_graph_id = None;
        self.graphs.live_snarl_graph_id = None;
        self.graphs.live_snarl = None;
        self.graphs.live_snarl_needs_rebuild = false;
        self.graphs.runtime_node_values.clear();
        self.graphs.preview_frames_by_graph.clear();
        self.graphs.plot_history.clear();
        self.ui.sink_preview_window_open = false;
        self.ui.sink_preview_selected_item_keys.clear();
        self.ui.sink_preview_selection_context_key = None;
        self.ui.sink_preview_display_scope_node_id = None;
        self.ui.diagnostics_window_graph_id = None;
        self.ui.diagnostics_window_node_id = None;
        #[cfg(target_arch = "wasm32")]
        {
            self.ui.browser_clipboard_events = None;
        }
    }

    /// Clears autosave bookkeeping for pending local graph edits.
    pub(crate) fn clear_pending_graph_update_tracking(&mut self) {
        self.graphs.pending_graph_update = false;
        self.graphs.graph_update_dirty_since_secs = None;
        self.graphs.graph_update_last_change_secs = None;
        self.graphs.graph_update_last_observed_document = None;
    }

    /// Resets all subscription bookkeeping so it can be rebuilt against the current connection.
    pub(crate) fn reset_subscription_sync_state(&mut self) {
        self.subscriptions.initialized = false;
        self.subscriptions.metadata_requested_once = false;
        self.subscriptions.node_definitions_requested_once = false;
        self.subscriptions.running_graphs_requested_once = false;
        self.subscriptions.runtime_graph_subscription = None;
        self.subscriptions.diagnostics_graph_subscriptions.clear();
        self.subscriptions.diagnostics_node_subscription = None;
        self.subscriptions.wled_instances_requested_once = false;
        self.subscriptions.mqtt_brokers_requested_once = false;
        self.subscriptions.active_event_subscriptions.clear();
    }
}

/// Converts a runtime-update payload into the editor's generic `(name, value)` representation.
fn decode_runtime_update_value(value: NodeRuntimeUpdateValue) -> (String, InputValue) {
    match value {
        NodeRuntimeUpdateValue::Inline { name, value } => (name, value),
    }
}

impl FrontendApp {
    fn refresh_graph_preview_frames(&mut self, graph_id: &str) {
        let Some(document) = self.graphs.loaded_graph_document.as_ref() else {
            return;
        };
        let mut frames = Vec::new();
        for node in &document.nodes {
            let Some(values) = self.graphs.runtime_node_values.get(&node.id) else {
                continue;
            };
            frames.extend(values.iter().filter_map(|(_name, value)| {
                preview_frame_from_input_value(value, node.id.as_str(), node.metadata.name.as_str())
            }));
        }
        self.graphs
            .preview_frames_by_graph
            .insert(graph_id.to_owned(), frames);
    }
}

fn preview_frame_from_input_value(
    value: &InputValue,
    source_node_id: &str,
    source_display_name: &str,
) -> Option<SinkPreviewFrame> {
    match value {
        InputValue::ColorFrame(frame) | InputValue::MappedFrame(frame)
            if frame.layout.points_3d.is_some() =>
        {
            Some(SinkPreviewFrame {
                sink_node_id: source_node_id.to_owned(),
                sink_node_name: source_display_name.to_owned(),
                layout: frame.layout.clone(),
                pixels: frame.pixels.clone(),
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, VecDeque};

    use shared::{
        ColorFrame, GraphDocument, GraphNode, GraphRuntimeMode, GraphRuntimeStatus, LedLayout,
        NodeDiagnosticEntry, NodeDiagnosticSeverity, NodeDiagnosticSummary, NodeMetadata,
        NodeRuntimeUpdateValue, NodeTypeId, NodeViewport, RgbaColor, SinkPreviewFrame, Vec3,
    };

    use crate::app::FrontendApp;

    fn graph_document_with_node_title(title: &str) -> GraphDocument {
        GraphDocument {
            metadata: shared::GraphMetadata {
                id: "graph-a".to_owned(),
                name: "Graph A".to_owned(),
                execution_frequency_hz: 60,
            },
            nodes: vec![GraphNode {
                id: "node-1".to_owned(),
                metadata: NodeMetadata {
                    name: title.to_owned(),
                },
                node_type: NodeTypeId::new("test.unknown"),
                viewport: NodeViewport::default(),
                input_values: Vec::new(),
                parameters: Vec::new(),
            }],
            ..GraphDocument::default()
        }
    }

    #[test]
    fn graph_diagnostics_are_cached_by_graph_id() {
        let mut app = FrontendApp::default();

        app.apply_graph_diagnostics_summary(
            "graph-a".to_owned(),
            vec![NodeDiagnosticSummary {
                node_id: "node-1".to_owned(),
                highest_severity: NodeDiagnosticSeverity::Error,
                active_count: 1,
            }],
        );
        app.apply_node_diagnostics_detail(
            "graph-a".to_owned(),
            "node-1".to_owned(),
            vec![NodeDiagnosticEntry {
                severity: NodeDiagnosticSeverity::Error,
                code: Some("runtime.unknown_node_type".to_owned()),
                message: "unknown node".to_owned(),
                occurrences: 1,
            }],
        );

        assert_eq!(
            app.graph_diagnostic_summaries("graph-a")
                .and_then(|nodes| nodes.get("node-1"))
                .map(|summary| summary.active_count),
            Some(1)
        );
        assert_eq!(app.node_diagnostic_details("graph-a", "node-1").len(), 1);
    }

    #[test]
    fn save_acknowledgement_resyncs_cached_live_snarl() {
        let mut app = FrontendApp::default();
        let local_document = graph_document_with_node_title("Local Title");
        app.graphs.loaded_graph_document = Some(local_document);
        app.rebuild_live_snarl_from_loaded_document();

        let acknowledged_document = graph_document_with_node_title("Backend Title");
        app.acknowledge_graph_save(acknowledged_document, true);

        let live_snarl = app.graphs.live_snarl.as_ref().expect("live snarl");
        let node_titles = crate::editor_view::snarl_node_titles(live_snarl);
        assert_eq!(node_titles, vec!["Backend Title".to_owned()]);
    }

    #[test]
    fn dummy_display_runtime_updates_refresh_preview_frames() {
        let mut app = FrontendApp::default();
        let mut document = graph_document_with_node_title("Dummy Graph");
        document.nodes[0].node_type = NodeTypeId::new(shared::NodeTypeId::WLED_DUMMY_DISPLAY);
        document.nodes[0].id = "dummy-1".to_owned();
        app.graphs.loaded_graph_document = Some(document);

        app.apply_runtime_update(
            "graph-a".to_owned(),
            "dummy-1".to_owned(),
            vec![NodeRuntimeUpdateValue::Inline {
                name: "frame".to_owned(),
                value: shared::InputValue::ColorFrame(ColorFrame {
                    layout: LedLayout {
                        id: "dummy-1".to_owned(),
                        role: ::shared::LedLayoutRole::RenderTarget,
                        pixel_count: 1,
                        width: Some(1),
                        height: Some(1),
                        points_3d: Some(vec![Vec3 {
                            x: 1.0,
                            y: 2.0,
                            z: 3.0,
                        }]),
                    },
                    pixels: vec![RgbaColor {
                        r: 1.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    }],
                }),
            }],
        );
        assert_eq!(
            app.graphs
                .preview_frames_by_graph
                .get("graph-a")
                .map(Vec::len),
            Some(1)
        );

        app.apply_runtime_update(
            "graph-a".to_owned(),
            "dummy-1".to_owned(),
            vec![NodeRuntimeUpdateValue::Inline {
                name: "frame".to_owned(),
                value: shared::InputValue::ColorFrame(ColorFrame {
                    layout: LedLayout {
                        id: "dummy-1".to_owned(),
                        role: ::shared::LedLayoutRole::RenderTarget,
                        pixel_count: 1,
                        width: Some(1),
                        height: Some(1),
                        points_3d: Some(vec![Vec3 {
                            x: 1.0,
                            y: 2.0,
                            z: 3.0,
                        }]),
                    },
                    pixels: vec![RgbaColor {
                        r: 0.0,
                        g: 1.0,
                        b: 0.0,
                        a: 1.0,
                    }],
                }),
            }],
        );

        assert_eq!(
            app.graphs
                .preview_frames_by_graph
                .get("graph-a")
                .and_then(|entries| entries.first())
                .map(|entry| entry.pixels[0]),
            Some(RgbaColor {
                r: 0.0,
                g: 1.0,
                b: 0.0,
                a: 1.0,
            })
        );
    }

    #[test]
    fn starting_selected_graph_clears_runtime_caches() {
        let mut app = FrontendApp::default();
        app.ui.selected_graph_id = Some("graph-a".to_owned());
        app.graphs
            .graph_runtime_modes
            .insert("graph-a".to_owned(), GraphRuntimeMode::Paused);
        app.graphs.runtime_node_values.insert(
            "node-1".to_owned(),
            HashMap::from([("value".to_owned(), shared::InputValue::Float(1.0))]),
        );
        app.graphs
            .plot_history
            .insert("node-1".to_owned(), VecDeque::from([1.0_f32, 2.0_f32]));
        app.graphs.preview_frames_by_graph.insert(
            "graph-a".to_owned(),
            vec![SinkPreviewFrame {
                sink_node_id: "node-1".to_owned(),
                sink_node_name: "Display".to_owned(),
                layout: LedLayout {
                    id: "layout-a".to_owned(),
                    role: shared::LedLayoutRole::RenderTarget,
                    pixel_count: 1,
                    width: Some(1),
                    height: Some(1),
                    points_3d: Some(vec![Vec3 {
                        x: 1.0,
                        y: 2.0,
                        z: 3.0,
                    }]),
                },
                pixels: vec![RgbaColor {
                    r: 1.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                }],
            }],
        );

        app.apply_runtime_statuses(vec![GraphRuntimeStatus {
            graph_id: "graph-a".to_owned(),
            mode: GraphRuntimeMode::Running,
        }]);

        assert!(app.graphs.runtime_node_values.is_empty());
        assert!(app.graphs.plot_history.is_empty());
        assert!(!app.graphs.preview_frames_by_graph.contains_key("graph-a"));
    }
}
