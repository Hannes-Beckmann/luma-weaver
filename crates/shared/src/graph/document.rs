use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use super::{InputValue, NodeTypeId};

/// Stores the stable identity and top-level metadata of a graph document.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GraphMetadata {
    pub id: String,
    pub name: String,
    #[serde(default = "default_execution_frequency_hz")]
    pub execution_frequency_hz: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
/// Represents a persisted graph document as edited in the frontend and stored by the backend.
pub struct GraphDocument {
    pub metadata: GraphMetadata,
    #[serde(default)]
    pub viewport: GraphViewport,
    #[serde(default)]
    pub nodes: Vec<GraphNode>,
    #[serde(default)]
    pub edges: Vec<GraphEdge>,
}

impl Default for GraphDocument {
    /// Builds an empty graph document with default viewport and execution frequency.
    fn default() -> Self {
        Self {
            metadata: GraphMetadata {
                id: String::new(),
                name: String::new(),
                execution_frequency_hz: default_execution_frequency_hz(),
            },
            viewport: GraphViewport::default(),
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
/// Represents one node instance inside a persisted graph document.
pub struct GraphNode {
    pub id: String,
    pub metadata: NodeMetadata,
    #[serde(default)]
    pub node_type: NodeTypeId,
    #[serde(default)]
    pub viewport: NodeViewport,
    #[serde(default)]
    pub input_values: Vec<NodeInputValue>,
    #[serde(default)]
    pub parameters: Vec<NodeParameter>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
/// Stores the editor camera state for a graph document.
pub struct GraphViewport {
    #[serde(default = "default_zoom")]
    pub zoom: f32,
    #[serde(default)]
    pub pan: ViewportPan,
}

impl Default for GraphViewport {
    /// Builds the default graph viewport centered at the origin and zoomed to `1.0`.
    fn default() -> Self {
        Self {
            zoom: default_zoom(),
            pan: ViewportPan::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
/// Stores the graph-canvas pan offset.
pub struct ViewportPan {
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
}

impl Default for ViewportPan {
    /// Builds a zero pan offset.
    fn default() -> Self {
        Self { x: 0.0, y: 0.0 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
/// Stores a node position in graph-canvas coordinates.
pub struct NodePosition {
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
}

impl Default for NodePosition {
    /// Builds the default node position at the origin.
    fn default() -> Self {
        Self { x: 0.0, y: 0.0 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
/// Stores per-node editor state such as position and collapse state.
pub struct NodeViewport {
    #[serde(default)]
    pub position: NodePosition,
    #[serde(default)]
    pub collapsed: bool,
}

impl Default for NodeViewport {
    /// Builds the default expanded node viewport at the origin.
    fn default() -> Self {
        Self {
            position: NodePosition::default(),
            collapsed: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// Stores editor-facing metadata for a node instance.
pub struct NodeMetadata {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
/// Stores a disconnected input value persisted directly on a graph node.
pub struct NodeInputValue {
    pub name: String,
    pub value: InputValue,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
/// Stores a serialized parameter value persisted directly on a graph node.
pub struct NodeParameter {
    pub name: String,
    pub value: JsonValue,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// Represents one directed connection between an output port and an input port.
pub struct GraphEdge {
    pub from_node_id: String,
    pub from_output_name: String,
    pub to_node_id: String,
    pub to_input_name: String,
}

/// Returns the default graph execution frequency used when the field is omitted from persisted data.
fn default_execution_frequency_hz() -> u32 {
    60
}

/// Returns the default graph viewport zoom used when the field is omitted from persisted data.
fn default_zoom() -> f32 {
    1.0
}
