use std::collections::HashMap;

use ::shared::{InputValue, LedLayout, NodeDiagnostic};
use serde::Serialize;

use super::serialize_outputs;

/// Carries per-evaluation runtime context that is shared across all node executions in a tick.
pub(crate) struct NodeEvaluationContext {
    pub(crate) graph_id: String,
    pub(crate) graph_name: String,
    pub(crate) elapsed_seconds: f64,
    pub(crate) render_layout: Option<LedLayout>,
}

/// Represents the untyped result of evaluating a runtime node.
///
/// Outputs are already serialized into named `InputValue`s so the executor can store and route
/// them without knowing the node's concrete Rust output type.
pub(crate) struct NodeEvaluation {
    pub(crate) outputs: HashMap<String, InputValue>,
    pub(crate) frontend_updates: Vec<NodeFrontendUpdate>,
    pub(crate) diagnostics: Vec<NodeDiagnostic>,
}

/// Describes a runtime-only value that should be pushed to the frontend without becoming a graph output.
pub(crate) struct NodeFrontendUpdate {
    pub(crate) name: String,
    pub(crate) value: InputValue,
}

/// Represents the typed result of evaluating a runtime node before output serialization.
pub(crate) struct TypedNodeEvaluation<Outputs> {
    pub(crate) outputs: Outputs,
    pub(crate) frontend_updates: Vec<NodeFrontendUpdate>,
    pub(crate) diagnostics: Vec<NodeDiagnostic>,
}

impl<Outputs> TypedNodeEvaluation<Outputs> {
    /// Builds a typed evaluation result containing only node outputs.
    pub(crate) fn from_outputs(outputs: Outputs) -> Self {
        Self {
            outputs,
            frontend_updates: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    /// Builds a typed evaluation result with additional frontend-only update payloads.
    pub(crate) fn with_frontend_updates(
        outputs: Outputs,
        frontend_updates: Vec<NodeFrontendUpdate>,
    ) -> Self {
        Self {
            outputs,
            frontend_updates,
            diagnostics: Vec::new(),
        }
    }
}

impl<Outputs> From<TypedNodeEvaluation<Outputs>> for NodeEvaluation
where
    Outputs: Serialize,
{
    /// Serializes typed outputs into the generic runtime representation used by the executor.
    fn from(value: TypedNodeEvaluation<Outputs>) -> Self {
        Self {
            outputs: serialize_outputs(value.outputs).expect("typed node outputs must serialize"),
            frontend_updates: value.frontend_updates,
            diagnostics: value.diagnostics,
        }
    }
}
