use super::{InputValue, NodeTypeId, Vec3};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashSet;

/// Stores the stable identity and top-level metadata of a graph document.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GraphMetadata {
    pub id: String,
    pub name: String,
    #[serde(default = "default_execution_frequency_hz")]
    pub execution_frequency_hz: u32,
    #[serde(default)]
    pub home_assistant_broker_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
/// Represents a persisted graph document as edited in the frontend and stored by the backend.
pub struct GraphDocument {
    pub metadata: GraphMetadata,
    #[serde(default)]
    pub viewport: GraphViewport,
    #[serde(default)]
    pub layout_assets: Vec<EmbeddedLayoutAsset>,
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
                home_assistant_broker_id: String::new(),
            },
            viewport: GraphViewport::default(),
            layout_assets: Vec::new(),
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
/// Stores one graph-owned embedded spatial layout asset.
pub struct EmbeddedLayoutAsset {
    pub id: String,
    #[serde(default)]
    pub points: Vec<Vec3>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Classifies a validation-style parse error while decoding an uploaded spatial layout.
pub enum LayoutParseErrorKind {
    Empty,
    InvalidUtf8,
    InvalidJson,
    InvalidCsv,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Reports why uploaded layout bytes could not be parsed into embedded 3D points.
pub struct LayoutParseError {
    pub kind: LayoutParseErrorKind,
    pub message: String,
}

impl std::fmt::Display for LayoutParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for LayoutParseError {}

/// Returns the default graph execution frequency used when the field is omitted from persisted data.
fn default_execution_frequency_hz() -> u32 {
    60
}

/// Returns the default graph viewport zoom used when the field is omitted from persisted data.
fn default_zoom() -> f32 {
    1.0
}

/// Returns whether a node parameter name references an embedded spatial layout asset.
pub fn is_layout_asset_parameter_name(name: &str) -> bool {
    name.ends_with("layout_asset_id")
}

/// Returns the referenced embedded layout id for the named parameter, if present.
pub fn layout_asset_id_from_parameter(parameter: &NodeParameter) -> Option<&str> {
    is_layout_asset_parameter_name(&parameter.name)
        .then(|| parameter.value.as_str())
        .flatten()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

impl GraphDocument {
    /// Returns the embedded layout points referenced by `layout_asset_id`, if the graph owns them.
    pub fn embedded_layout_points(&self, layout_asset_id: &str) -> Option<&[Vec3]> {
        self.layout_assets
            .iter()
            .find(|asset| asset.id == layout_asset_id)
            .map(|asset| asset.points.as_slice())
    }

    /// Returns every embedded layout id currently referenced by graph node parameters.
    pub fn referenced_layout_asset_ids(&self) -> HashSet<String> {
        self.nodes
            .iter()
            .flat_map(|node| node.parameters.iter())
            .filter_map(layout_asset_id_from_parameter)
            .map(str::to_owned)
            .collect()
    }

    /// Removes embedded layouts that are no longer referenced by any node parameter.
    pub fn prune_unreferenced_layout_assets(&mut self) {
        let referenced_ids = self.referenced_layout_asset_ids();
        self.layout_assets
            .retain(|asset| referenced_ids.contains(asset.id.as_str()));
    }
}

/// Parses uploaded spatial layout bytes into a list of 3D points suitable for embedding in a graph.
pub fn parse_layout_points(bytes: &[u8]) -> Result<Vec<Vec3>, LayoutParseError> {
    let trimmed = std::str::from_utf8(bytes)
        .map_err(|_| LayoutParseError {
            kind: LayoutParseErrorKind::InvalidUtf8,
            message: "decode uploaded layout as UTF-8".to_owned(),
        })?
        .trim();
    if trimmed.is_empty() {
        return Err(LayoutParseError {
            kind: LayoutParseErrorKind::Empty,
            message: "uploaded layout is empty".to_owned(),
        });
    }

    if trimmed.starts_with('[') {
        let points = serde_json::from_str::<Vec<Vec3>>(trimmed).map_err(|error| LayoutParseError {
            kind: LayoutParseErrorKind::InvalidJson,
            message: format!("parse uploaded layout JSON point array: {error}"),
        })?;
        if points.is_empty() {
            return Err(LayoutParseError {
                kind: LayoutParseErrorKind::Empty,
                message: "uploaded layout contains no points".to_owned(),
            });
        }
        return Ok(points);
    }

    parse_layout_csv(trimmed)
}

fn parse_layout_csv(csv_text: &str) -> Result<Vec<Vec3>, LayoutParseError> {
    let mut lines = csv_text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty());
    let Some(header) = lines.next() else {
        return Err(LayoutParseError {
            kind: LayoutParseErrorKind::Empty,
            message: "uploaded layout CSV is empty".to_owned(),
        });
    };
    let columns = header
        .split(',')
        .map(|column| column.trim().to_ascii_lowercase())
        .collect::<Vec<_>>();
    if columns != ["x", "y", "z"] {
        return Err(LayoutParseError {
            kind: LayoutParseErrorKind::InvalidCsv,
            message: "uploaded layout CSV must use exactly the headers x,y,z".to_owned(),
        });
    }

    let mut points = Vec::new();
    for (line_index, line) in lines.enumerate() {
        let values = line.split(',').map(str::trim).collect::<Vec<_>>();
        if values.len() != 3 {
            return Err(LayoutParseError {
                kind: LayoutParseErrorKind::InvalidCsv,
                message: format!(
                    "uploaded layout CSV row {} must contain exactly 3 columns",
                    line_index + 2
                ),
            });
        }
        let parse = |value: &str, axis: &str| {
            value.parse::<f32>().map_err(|error| LayoutParseError {
                kind: LayoutParseErrorKind::InvalidCsv,
                message: format!("parse uploaded layout CSV {axis} value '{value}': {error}"),
            })
        };
        points.push(Vec3 {
            x: parse(values[0], "x")?,
            y: parse(values[1], "y")?,
            z: parse(values[2], "z")?,
        });
    }

    if points.is_empty() {
        return Err(LayoutParseError {
            kind: LayoutParseErrorKind::Empty,
            message: "uploaded layout contains no points".to_owned(),
        });
    }
    Ok(points)
}
