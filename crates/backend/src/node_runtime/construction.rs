use std::collections::HashMap;

use ::shared::NodeDiagnostic;
use serde_json::Value as JsonValue;

/// Constructs a runtime node instance from serialized graph parameters.
///
/// Implementations may normalize incoming parameter values and emit one-time construction
/// diagnostics that explain defaults, clamping, or other adjustments performed before runtime
/// evaluation begins.
pub(crate) trait RuntimeNodeFromParameters: Sized + 'static + Default {
    /// Builds a node instance and any construction-time diagnostics from raw parameter values.
    ///
    /// The default implementation ignores all parameters and constructs `Self::default()`
    /// without diagnostics.
    fn from_parameters(_parameters: &HashMap<String, JsonValue>) -> NodeConstruction<Self> {
        NodeConstruction {
            node: Self::default(),
            diagnostics: Vec::new(),
        }
    }
}

/// Holds the constructed runtime node alongside any diagnostics produced during construction.
pub(crate) struct NodeConstruction<T> {
    pub(crate) node: T,
    pub(crate) diagnostics: Vec<NodeDiagnostic>,
}
