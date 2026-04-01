use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use shared::{
    EventMessage, GraphMetadata, GraphRuntimeStatus, NodeDiagnostic, NodeDiagnosticEntry,
    NodeDiagnosticSeverity, NodeDiagnosticSummary, NodeRuntimeValue, WledInstance,
};
use tokio::sync::broadcast;

use crate::services::graph_store::GraphStoreEventPublisher;
use crate::services::runtime::types::RuntimeEventPublisher;

#[derive(Clone)]
/// Broadcasts backend events to connected clients and stores persistent node diagnostics.
pub(crate) struct EventBus {
    sender: broadcast::Sender<BackendEvent>,
    diagnostics: Arc<
        RwLock<
            HashMap<String, HashMap<String, HashMap<DiagnosticFingerprint, NodeDiagnosticEntry>>>,
        >,
    >,
}

#[derive(Clone, Debug)]
/// Represents every backend event type that can be fanned out to WebSocket clients.
pub(crate) enum BackendEvent {
    EventMessage(EventMessage),
    GraphMetadataChanged {
        documents: Vec<GraphMetadata>,
    },
    RuntimeStatusesChanged {
        statuses: Vec<GraphRuntimeStatus>,
    },
    NodeRuntimeUpdate {
        graph_id: String,
        node_id: String,
        values: Vec<NodeRuntimeValue>,
    },
    GraphDiagnosticsSummaryChanged {
        graph_id: String,
        nodes: Vec<NodeDiagnosticSummary>,
    },
    NodeDiagnosticsDetailChanged {
        graph_id: String,
        node_id: String,
        diagnostics: Vec<NodeDiagnosticEntry>,
    },
    WledInstancesChanged {
        instances: Vec<WledInstance>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
/// Identifies a logical diagnostic entry independently of its occurrence count.
struct DiagnosticFingerprint {
    severity: NodeDiagnosticSeverity,
    code: Option<String>,
    kind: String,
}

impl Default for EventBus {
    /// Builds an empty event bus with a bounded broadcast channel and no stored diagnostics.
    fn default() -> Self {
        let (sender, _) = broadcast::channel(256);
        Self {
            sender,
            diagnostics: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl EventBus {
    /// Creates a new receiver subscribed to the backend event broadcast stream.
    pub(crate) fn subscribe(&self) -> broadcast::Receiver<BackendEvent> {
        self.sender.subscribe()
    }

    /// Broadcasts a generic backend event message.
    pub(crate) fn emit_event(&self, event: EventMessage) {
        tracing::trace!(topic = ?event.topic, scope = ?event.scope, "emitting backend event");
        let _ = self.sender.send(BackendEvent::EventMessage(event));
    }

    /// Broadcasts an updated graph metadata snapshot.
    pub(crate) fn emit_graph_metadata_changed(&self, documents: Vec<GraphMetadata>) {
        tracing::trace!("emitting graph metadata update");
        let _ = self
            .sender
            .send(BackendEvent::GraphMetadataChanged { documents });
    }

    /// Broadcasts updated runtime statuses for all managed graphs.
    pub(crate) fn emit_runtime_statuses_changed(&self, statuses: Vec<GraphRuntimeStatus>) {
        tracing::trace!("emitting runtime status update");
        let _ = self
            .sender
            .send(BackendEvent::RuntimeStatusesChanged { statuses });
    }

    /// Broadcasts a node runtime update when it contains at least one value.
    pub(crate) fn emit_node_runtime_update(
        &self,
        graph_id: String,
        node_id: String,
        values: Vec<NodeRuntimeValue>,
    ) {
        if values.is_empty() {
            return;
        }
        let _ = self.sender.send(BackendEvent::NodeRuntimeUpdate {
            graph_id,
            node_id,
            values,
        });
    }

    /// Records node diagnostics, merges repeated entries, and broadcasts fresh summary and detail views.
    pub(crate) fn record_node_diagnostics(
        &self,
        graph_id: String,
        node_id: String,
        diagnostics: Vec<NodeDiagnostic>,
    ) {
        if diagnostics.is_empty() {
            return;
        }

        let (summary, detail) = {
            let mut graphs = self.diagnostics.write().expect("diagnostics lock poisoned");
            let nodes = graphs.entry(graph_id.clone()).or_default();
            {
                let entries = nodes.entry(node_id.clone()).or_default();
                for diagnostic in diagnostics {
                    let fingerprint = DiagnosticFingerprint {
                        severity: diagnostic.severity,
                        code: diagnostic.code.clone(),
                        kind: diagnostic
                            .code
                            .clone()
                            .unwrap_or_else(|| diagnostic.message.clone()),
                    };
                    entries
                        .entry(fingerprint)
                        .and_modify(|entry| {
                            entry.occurrences = entry.occurrences.saturating_add(1);
                            entry.message = diagnostic.message.clone();
                        })
                        .or_insert(NodeDiagnosticEntry {
                            severity: diagnostic.severity,
                            code: diagnostic.code,
                            message: diagnostic.message,
                            occurrences: 1,
                        });
                }
            }

            let detail = nodes.get(&node_id).map(detail_for_node).unwrap_or_default();
            (summaries_for_graph(nodes), detail)
        };

        let _ = self
            .sender
            .send(BackendEvent::GraphDiagnosticsSummaryChanged {
                graph_id: graph_id.clone(),
                nodes: summary,
            });
        let _ = self
            .sender
            .send(BackendEvent::NodeDiagnosticsDetailChanged {
                graph_id,
                node_id,
                diagnostics: detail,
            });
    }

    /// Returns the current diagnostic summary list for a graph.
    pub(crate) fn graph_diagnostics_summary(&self, graph_id: &str) -> Vec<NodeDiagnosticSummary> {
        let graphs = self.diagnostics.read().expect("diagnostics lock poisoned");
        graphs
            .get(graph_id)
            .map(summaries_for_graph)
            .unwrap_or_default()
    }

    /// Returns the current detailed diagnostic entries for a node.
    pub(crate) fn node_diagnostics_detail(
        &self,
        graph_id: &str,
        node_id: &str,
    ) -> Vec<NodeDiagnosticEntry> {
        let graphs = self.diagnostics.read().expect("diagnostics lock poisoned");
        graphs
            .get(graph_id)
            .and_then(|nodes| nodes.get(node_id))
            .map(detail_for_node)
            .unwrap_or_default()
    }

    /// Clears all stored diagnostics for a node and broadcasts the resulting empty detail view.
    pub(crate) fn clear_node_diagnostics(&self, graph_id: &str, node_id: &str) {
        let summary = {
            let mut graphs = self.diagnostics.write().expect("diagnostics lock poisoned");
            if let Some(nodes) = graphs.get_mut(graph_id) {
                nodes.remove(node_id);
                let summary = summaries_for_graph(nodes);
                if nodes.is_empty() {
                    graphs.remove(graph_id);
                }
                summary
            } else {
                Vec::new()
            }
        };

        let _ = self
            .sender
            .send(BackendEvent::GraphDiagnosticsSummaryChanged {
                graph_id: graph_id.to_owned(),
                nodes: summary,
            });
        let _ = self
            .sender
            .send(BackendEvent::NodeDiagnosticsDetailChanged {
                graph_id: graph_id.to_owned(),
                node_id: node_id.to_owned(),
                diagnostics: Vec::new(),
            });
    }

    /// Broadcasts the latest discovered WLED instance list.
    pub(crate) fn emit_wled_instances_changed(&self, instances: Vec<WledInstance>) {
        let _ = self
            .sender
            .send(BackendEvent::WledInstancesChanged { instances });
    }
}

impl RuntimeEventPublisher for EventBus {
    /// Publishes the runtime-manager status snapshot through the event bus.
    fn runtime_statuses_changed(&self, statuses: Vec<GraphRuntimeStatus>) {
        self.emit_runtime_statuses_changed(statuses);
    }

    /// Publishes node runtime values through the event bus.
    fn node_runtime_update(
        &self,
        graph_id: String,
        node_id: String,
        values: Vec<NodeRuntimeValue>,
    ) {
        self.emit_node_runtime_update(graph_id, node_id, values);
    }

    /// Publishes node diagnostics through the event bus's persistent diagnostic store.
    fn node_diagnostics(
        &self,
        graph_id: String,
        node_id: String,
        diagnostics: Vec<NodeDiagnostic>,
    ) {
        self.record_node_diagnostics(graph_id, node_id, diagnostics);
    }
}

impl GraphStoreEventPublisher for EventBus {
    /// Publishes graph metadata changes produced by the graph store.
    fn graph_metadata_changed(&self, documents: Vec<GraphMetadata>) {
        self.emit_graph_metadata_changed(documents);
    }
}

/// Builds the per-node diagnostic summary list for a graph from stored detailed entries.
fn summaries_for_graph(
    nodes: &HashMap<String, HashMap<DiagnosticFingerprint, NodeDiagnosticEntry>>,
) -> Vec<NodeDiagnosticSummary> {
    let mut summaries = nodes
        .iter()
        .filter_map(|(node_id, entries)| {
            let highest_severity = entries.values().map(|entry| entry.severity).max()?;
            Some(NodeDiagnosticSummary {
                node_id: node_id.clone(),
                highest_severity,
                active_count: entries.len(),
            })
        })
        .collect::<Vec<_>>();
    summaries.sort_by(|left, right| left.node_id.cmp(&right.node_id));
    summaries
}

/// Returns the sorted detailed diagnostic entries for a single node.
fn detail_for_node(
    entries: &HashMap<DiagnosticFingerprint, NodeDiagnosticEntry>,
) -> Vec<NodeDiagnosticEntry> {
    let mut detail = entries.values().cloned().collect::<Vec<_>>();
    detail.sort_by(|left, right| {
        right
            .severity
            .cmp(&left.severity)
            .then_with(|| left.code.cmp(&right.code))
            .then_with(|| left.message.cmp(&right.message))
    });
    detail
}
