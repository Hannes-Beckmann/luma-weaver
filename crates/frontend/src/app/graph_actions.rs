use eframe::egui;
use shared::{
    ClientMessage, GraphRuntimeMode, MqttBrokerConfig, NodeDiagnosticEntry, NodeDiagnosticSummary,
};

use super::FrontendApp;

impl FrontendApp {
    /// Requests that the backend start or resume execution for the given graph.
    pub(crate) fn start_graph(&mut self, graph_id: String) {
        self.send(ClientMessage::StartGraph { id: graph_id });
    }

    /// Requests that the backend pause execution for the given graph.
    pub(crate) fn pause_graph(&mut self, graph_id: String) {
        self.send(ClientMessage::PauseGraph { id: graph_id });
    }

    /// Requests a bounded manual step for a paused graph.
    pub(crate) fn step_graph(&mut self, graph_id: String, ticks: u32) {
        self.send(ClientMessage::StepGraph {
            id: graph_id,
            ticks,
        });
    }

    /// Requests that the backend stop execution for the given graph.
    pub(crate) fn stop_graph(&mut self, graph_id: String) {
        self.send(ClientMessage::StopGraph { id: graph_id });
    }

    /// Updates the locally cached and persisted execution frequency for a graph.
    ///
    /// The value is clamped to at least `1 Hz` before it is reflected in local UI state and sent
    /// to the backend.
    pub(crate) fn update_graph_execution_frequency(
        &mut self,
        graph_id: String,
        execution_frequency_hz: u32,
    ) {
        let execution_frequency_hz = execution_frequency_hz.max(1);

        if let Some(graph) = self
            .graphs
            .graph_documents
            .iter_mut()
            .find(|graph| graph.id == graph_id)
        {
            graph.execution_frequency_hz = execution_frequency_hz;
        }

        if self
            .graphs
            .loaded_graph_document
            .as_ref()
            .map(|document| document.metadata.id.as_str())
            == Some(graph_id.as_str())
        {
            if let Some(document) = self.graphs.loaded_graph_document.as_mut() {
                document.metadata.execution_frequency_hz = execution_frequency_hz;
            }
        }

        self.send(ClientMessage::UpdateGraphExecutionFrequency {
            id: graph_id,
            execution_frequency_hz,
        });
    }

    /// Updates the locally cached graph name and sends the rename request to the backend.
    ///
    /// Empty names are rejected immediately so the user gets fast feedback without waiting for the
    /// server round-trip.
    pub(crate) fn update_graph_name(&mut self, graph_id: String, name: String) {
        let trimmed_name = name.trim().to_owned();
        if trimmed_name.is_empty() {
            self.ui.status = "Graph document name must not be empty".to_owned();
            return;
        }

        if let Some(graph) = self
            .graphs
            .graph_documents
            .iter_mut()
            .find(|graph| graph.id == graph_id)
        {
            graph.name = trimmed_name.clone();
        }

        if self
            .graphs
            .loaded_graph_document
            .as_ref()
            .map(|document| document.metadata.id.as_str())
            == Some(graph_id.as_str())
        {
            if let Some(document) = self.graphs.loaded_graph_document.as_mut() {
                document.metadata.name = trimmed_name.clone();
            }
        }

        self.ui.rename_graph_name = trimmed_name.clone();
        self.send(ClientMessage::UpdateGraphName {
            id: graph_id,
            name: trimmed_name,
        });
    }

    /// Persists the full MQTT broker configuration list through the backend.
    pub(crate) fn save_mqtt_broker_configs(&mut self, brokers: Vec<MqttBrokerConfig>) {
        self.send(ClientMessage::UpdateMqttBrokerConfigs { brokers });
    }

    /// Opens the diagnostics window for a specific node in the editor.
    pub(crate) fn open_node_diagnostics(&mut self, node_id: String) {
        self.ui.diagnostics_window_graph_id = self.ui.selected_graph_id.clone();
        self.ui.diagnostics_window_node_id = Some(node_id);
    }

    /// Opens the diagnostics window for a specific graph/node without changing the active view.
    pub(crate) fn open_graph_diagnostics(&mut self, graph_id: String, node_id: String) {
        self.ui.diagnostics_window_graph_id = Some(graph_id);
        self.ui.diagnostics_window_node_id = Some(node_id);
    }

    /// Closes the currently open node diagnostics window.
    pub(crate) fn close_node_diagnostics(&mut self) {
        self.ui.diagnostics_window_graph_id = None;
        self.ui.diagnostics_window_node_id = None;
    }

    /// Requests that the backend clear all persistent diagnostics for a node.
    pub(crate) fn clear_node_diagnostics(&mut self, graph_id: String, node_id: String) {
        self.send(ClientMessage::ClearNodeDiagnostics { graph_id, node_id });
    }

    /// Returns the last runtime mode reported for the given graph.
    pub(crate) fn graph_runtime_mode(&self, graph_id: &str) -> Option<GraphRuntimeMode> {
        self.graphs.graph_runtime_modes.get(graph_id).copied()
    }

    /// Returns the cached diagnostic summary map for a graph.
    pub(crate) fn graph_diagnostic_summaries(
        &self,
        graph_id: &str,
    ) -> Option<&std::collections::HashMap<String, NodeDiagnosticSummary>> {
        self.graphs.node_diagnostic_summaries_by_graph.get(graph_id)
    }

    /// Returns the cached detailed diagnostics for one node in a graph.
    pub(crate) fn node_diagnostic_details(
        &self,
        graph_id: &str,
        node_id: &str,
    ) -> &[NodeDiagnosticEntry] {
        self.graphs
            .node_diagnostic_details_by_graph
            .get(graph_id)
            .and_then(|nodes| nodes.get(node_id))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Returns the selected graph name, if a graph is currently open or selected.
    pub(crate) fn current_graph_name(&self) -> Option<&str> {
        self.selected_graph().map(|graph| graph.name.as_str())
    }

    /// Returns the short save-state label shown in the header.
    pub(crate) fn graph_save_status_label(&self) -> &'static str {
        if self.graphs.save_in_flight_document.is_some() {
            "Saving"
        } else if self.graphs.pending_graph_update {
            "Unsaved changes"
        } else if self.graphs.loaded_graph_document.is_some() {
            "All changes saved"
        } else if self.graphs.requested_graph_document_id.is_some() {
            "Loading graph"
        } else {
            "No graph loaded"
        }
    }

    /// Returns the short connection-state label shown in the header.
    pub(crate) fn websocket_status_label(&self) -> &'static str {
        if self.connection.sender.is_some() && self.connection.has_confirmed_connection {
            "Connected"
        } else if self.connection.sender.is_some() {
            "Connecting"
        } else {
            "Offline"
        }
    }

    /// Returns the current egui time in seconds for debounce and reconnection timers.
    pub(crate) fn now_secs(ctx: &egui::Context) -> f64 {
        ctx.input(|input| input.time)
    }
}
