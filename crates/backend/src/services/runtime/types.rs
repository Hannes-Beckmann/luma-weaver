use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde_json::Value as JsonValue;
use shared::{
    GraphRuntimeMode, GraphRuntimeStatus, InputValue, LedLayout, NodeDiagnostic, NodeRuntimeValue,
    NodeTypeId,
};

use crate::node_runtime::NodeRegistry;
use crate::node_runtime::RuntimeNodeEvaluator;

/// Publishes runtime-side status, value, and diagnostic updates back into the rest of the backend.
pub(crate) trait RuntimeEventPublisher: Send + Sync {
    /// Publishes the latest status snapshot for all runtime-managed graphs.
    fn runtime_statuses_changed(&self, statuses: Vec<GraphRuntimeStatus>);
    /// Publishes runtime-update values for a single node.
    fn node_runtime_update(&self, graph_id: String, node_id: String, values: Vec<NodeRuntimeValue>);
    /// Publishes the current diagnostics for a single node.
    fn node_diagnostics(&self, graph_id: String, node_id: String, diagnostics: Vec<NodeDiagnostic>);
}

/// Represents a compiled graph ready for runtime execution.
pub(crate) struct CompiledGraph {
    pub(crate) graph_id: String,
    pub(crate) graph_name: String,
    pub(crate) execution_frequency_hz: u32,
    pub(crate) node_registry: Arc<NodeRegistry>,
    pub(crate) nodes: Vec<CompiledNode>,
    pub(crate) incoming_edges_by_node: Vec<Vec<CompiledIncomingEdge>>,
    pub(crate) topological_order: Vec<usize>,
    pub(crate) render_contexts_by_node: Vec<Vec<RenderContext>>,
}

#[derive(Default)]
/// Stores mutable per-graph execution state that evolves across ticks.
pub(crate) struct GraphExecutionState {
    pub(crate) evaluators: HashMap<(usize, String), Box<dyn RuntimeNodeEvaluator>>,
    pub(crate) previous_outputs: HashMap<(usize, String, String), InputValue>,
    pub(crate) last_runtime_update_seconds: HashMap<(usize, String), f64>,
}

/// Represents a compiled node together with the metadata needed to execute it.
pub(crate) struct CompiledNode {
    pub(crate) id: String,
    pub(crate) node_type: NodeTypeId,
    pub(crate) input_defaults: HashMap<String, InputValue>,
    pub(crate) parameters: HashMap<String, JsonValue>,
    pub(crate) construction_diagnostics: Vec<NodeDiagnostic>,
    pub(crate) allowed_runtime_update_names: HashSet<String>,
}

#[derive(Clone)]
/// Represents a validated incoming edge in compiled form.
pub(crate) struct CompiledIncomingEdge {
    pub(crate) from_node_index: usize,
    pub(crate) from_output_name: String,
    pub(crate) to_input_name: String,
    pub(crate) use_previous_tick: bool,
}

#[derive(Debug, Clone)]
/// Describes a concrete render layout under which a node should be evaluated.
pub(crate) struct RenderContext {
    pub(crate) id: String,
    pub(crate) layout: LedLayout,
}

#[cfg(not(target_arch = "wasm32"))]
use tokio::sync::oneshot;

/// Wraps the latest runtime status snapshot returned by manager control operations.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) struct RuntimeStatusesUpdate {
    pub(crate) statuses: Vec<GraphRuntimeStatus>,
}

/// Holds the channels and current mode for a running graph task.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) struct RuntimeTask {
    pub(crate) mode: GraphRuntimeMode,
    pub(crate) stop_tx: oneshot::Sender<()>,
    pub(crate) command_tx: tokio::sync::mpsc::UnboundedSender<GraphRuntimeCommand>,
}

/// Command messages sent from the runtime manager into a graph execution task.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) enum GraphRuntimeCommand {
    Start,
    Pause,
    Step {
        ticks: usize,
        done_tx: oneshot::Sender<()>,
    },
}
