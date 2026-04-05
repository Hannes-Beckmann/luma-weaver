//! Shared WebSocket protocol and transport payload types used by the frontend and backend.
//!
//! This module defines the request/response contract for the structured JSON channel as well as
//! the compact binary frame transport used for large runtime frame updates.

use serde::{Deserialize, Serialize};

use crate::{
    events::{EventMessage, EventSubscription, ServerState},
    graph::{
        GraphDocument, GraphExchangeFile, GraphImportCollisionPolicy, GraphImportMode,
        GraphMetadata, InputValue, LedLayout, NodeDefinition,
    },
};

/// Describes a reusable MQTT broker configuration referenced by Home Assistant MQTT nodes.
///
/// These values are stored separately from graph documents so multiple nodes can share the same
/// broker connection details. Brokers can be marked as Home Assistant brokers so the frontend
/// and runtime can hide or ignore generic MQTT brokers where appropriate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MqttBrokerConfig {
    pub id: String,
    pub display_name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    #[serde(default = "default_mqtt_discovery_prefix")]
    pub discovery_prefix: String,
    #[serde(default = "default_home_assistant_broker")]
    pub is_home_assistant: bool,
}

/// Returns the default MQTT discovery prefix used for Home Assistant integration.
fn default_mqtt_discovery_prefix() -> String {
    "homeassistant".to_owned()
}

/// Returns the default broker intent for existing persisted broker configs.
fn default_home_assistant_broker() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "payload")]
/// Defines every request the frontend can send to the backend over the structured WebSocket channel.
///
/// Most variants are command or query messages. Runtime frame payloads are not sent this way;
/// those use the backend-to-frontend binary frame transport instead.
pub enum ClientMessage {
    Ping,
    SetName {
        name: String,
    },
    Subscribe {
        subscriptions: Vec<EventSubscription>,
    },
    Unsubscribe {
        subscriptions: Vec<EventSubscription>,
    },
    CreateGraphDocument {
        name: String,
    },
    DeleteGraphDocument {
        id: String,
    },
    GetGraphDocument {
        id: String,
    },
    ExportGraphDocument {
        id: String,
    },
    UpdateGraphDocument {
        document: GraphDocument,
    },
    UpdateGraphName {
        id: String,
        name: String,
    },
    ImportGraphDocument {
        file: GraphExchangeFile,
        collision_policy: GraphImportCollisionPolicy,
    },
    UpdateGraphExecutionFrequency {
        id: String,
        execution_frequency_hz: u32,
    },
    GetNodeDefinitions,
    GetGraphMetadata,
    StartGraph {
        id: String,
    },
    PauseGraph {
        id: String,
    },
    StepGraph {
        id: String,
        ticks: u32,
    },
    StopGraph {
        id: String,
    },
    GetRuntimeStatuses,
    SubscribeGraphRuntime {
        graph_id: String,
    },
    UnsubscribeGraphRuntime {
        graph_id: String,
    },
    SubscribeGraphDiagnostics {
        graph_id: String,
    },
    UnsubscribeGraphDiagnostics {
        graph_id: String,
    },
    SubscribeNodeDiagnostics {
        graph_id: String,
        node_id: String,
    },
    UnsubscribeNodeDiagnostics {
        graph_id: String,
        node_id: String,
    },
    ClearNodeDiagnostics {
        graph_id: String,
        node_id: String,
    },
    GetWledInstances,
    GetMqttBrokerConfigs,
    UpdateMqttBrokerConfigs {
        brokers: Vec<MqttBrokerConfig>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "payload")]
/// Defines every structured message the backend can send to the frontend.
///
/// This enum mixes:
/// - direct request responses such as `GraphDocument` or `MqttBrokerConfigs`
/// - state snapshots such as `State` and `RuntimeStatuses`
/// - streamed updates such as `Event` and `NodeRuntimeUpdate`
pub enum ServerMessage {
    Welcome {
        message: String,
    },
    State(ServerState),
    Pong,
    Error {
        message: String,
    },
    Event(EventMessage),
    SubscriptionState {
        subscriptions: Vec<EventSubscription>,
    },
    GraphMetadata {
        documents: Vec<GraphMetadata>,
    },
    NodeDefinitions {
        definitions: Vec<NodeDefinition>,
    },
    GraphDocument {
        document: GraphDocument,
    },
    GraphExport {
        file: GraphExchangeFile,
    },
    GraphImported {
        document: GraphDocument,
        mode: GraphImportMode,
    },
    RuntimeStatuses {
        graphs: Vec<GraphRuntimeStatus>,
    },
    NodeRuntimeUpdate {
        graph_id: String,
        node_id: String,
        values: Vec<NodeRuntimeUpdateValue>,
    },
    GraphDiagnosticsSummary {
        graph_id: String,
        nodes: Vec<NodeDiagnosticSummary>,
    },
    NodeDiagnosticsDetail {
        graph_id: String,
        node_id: String,
        diagnostics: Vec<NodeDiagnosticEntry>,
    },
    WledInstances {
        instances: Vec<WledInstance>,
    },
    MqttBrokerConfigs {
        brokers: Vec<MqttBrokerConfig>,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
/// Orders node diagnostics by severity for summaries, UI rendering, and sorting.
pub enum NodeDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
/// Represents one diagnostic emitted by a node during construction or evaluation.
///
/// The optional `code` is intended to be stable enough for UI grouping and user-dismissal logic,
/// while `message` remains the human-readable explanation.
pub struct NodeDiagnostic {
    pub severity: NodeDiagnosticSeverity,
    pub code: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// Summarizes the active diagnostic state of one node without including full messages.
pub struct NodeDiagnosticSummary {
    pub node_id: String,
    pub highest_severity: NodeDiagnosticSeverity,
    pub active_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// Represents one aggregated diagnostic entry shown in a node detail view.
///
/// Repeated diagnostics are merged by the backend and counted in `occurrences`.
pub struct NodeDiagnosticEntry {
    pub severity: NodeDiagnosticSeverity,
    pub code: Option<String>,
    pub message: String,
    pub occurrences: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
/// Represents one named runtime value before transport encoding choices are applied.
///
/// The backend may choose to send these values inline through JSON or through a specialized binary
/// channel, depending on the payload and transport rules.
pub struct NodeRuntimeValue {
    pub name: String,
    pub value: InputValue,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
/// Describes whether a graph runtime task is actively ticking or paused.
pub enum GraphRuntimeMode {
    Running,
    Paused,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// Reports the current runtime mode for a single graph.
pub struct GraphRuntimeStatus {
    pub graph_id: String,
    pub mode: GraphRuntimeMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "encoding", rename_all = "snake_case")]
/// Represents a runtime update after it has been prepared for structured WebSocket transport.
///
/// Large frame payloads may bypass this enum entirely and travel as `BinaryRuntimeFrameMessage`s.
pub enum NodeRuntimeUpdateValue {
    Inline { name: String, value: InputValue },
}

/// Magic bytes that identify the binary frame transport used for large runtime frame updates.
const BINARY_FRAME_MAGIC: [u8; 4] = *b"AFB1";
/// Current version of the binary frame message format.
const BINARY_FRAME_VERSION: u8 = 1;
/// Message kind tag for binary runtime frame updates.
const BINARY_FRAME_KIND_RUNTIME_UPDATE: u8 = 1;

#[derive(Debug, Clone, PartialEq)]
/// Encodes a frame runtime update in a compact binary format.
///
/// This format exists so large `ColorFrame` updates do not need to travel through the JSON
/// protocol path, while still round-tripping back into the same logical `ServerMessage` shape.
pub struct BinaryRuntimeFrameMessage {
    pub graph_id: String,
    pub node_id: String,
    pub name: String,
    pub layout: LedLayout,
    pub rgba: Vec<u8>,
}

impl BinaryRuntimeFrameMessage {
    /// Encodes the message into the shared binary frame wire format.
    pub fn encode(&self) -> Vec<u8> {
        let graph_id = self.graph_id.as_bytes();
        let node_id = self.node_id.as_bytes();
        let name = self.name.as_bytes();
        let layout_id = self.layout.id.as_bytes();
        let mut bytes = Vec::with_capacity(
            4 + 2
                + (5 * 4)
                + graph_id.len()
                + node_id.len()
                + name.len()
                + layout_id.len()
                + self.rgba.len(),
        );
        bytes.extend_from_slice(&BINARY_FRAME_MAGIC);
        bytes.push(BINARY_FRAME_VERSION);
        bytes.push(BINARY_FRAME_KIND_RUNTIME_UPDATE);
        push_u32(&mut bytes, graph_id.len());
        push_u32(&mut bytes, node_id.len());
        push_u32(&mut bytes, name.len());
        push_u32(&mut bytes, layout_id.len());
        push_u32(&mut bytes, self.layout.width.unwrap_or(0));
        push_u32(&mut bytes, self.layout.height.unwrap_or(0));
        push_u32(&mut bytes, self.layout.pixel_count);
        bytes.extend_from_slice(graph_id);
        bytes.extend_from_slice(node_id);
        bytes.extend_from_slice(name);
        bytes.extend_from_slice(layout_id);
        bytes.extend_from_slice(&self.rgba);
        bytes
    }

    /// Decodes a binary frame message from its wire representation.
    ///
    /// This validates the format magic, version, kind, declared lengths, and RGBA payload size.
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < 4 + 2 + (7 * 4) {
            return Err("binary frame packet too short".to_owned());
        }
        if bytes[..4] != BINARY_FRAME_MAGIC {
            return Err("binary frame packet magic mismatch".to_owned());
        }
        if bytes[4] != BINARY_FRAME_VERSION {
            return Err(format!(
                "binary frame packet version {} unsupported",
                bytes[4]
            ));
        }
        if bytes[5] != BINARY_FRAME_KIND_RUNTIME_UPDATE {
            return Err(format!("binary frame packet kind {} unsupported", bytes[5]));
        }

        let mut offset = 6usize;
        let graph_id_len = read_u32(bytes, &mut offset)? as usize;
        let node_id_len = read_u32(bytes, &mut offset)? as usize;
        let name_len = read_u32(bytes, &mut offset)? as usize;
        let layout_id_len = read_u32(bytes, &mut offset)? as usize;
        let width = read_u32(bytes, &mut offset)?;
        let height = read_u32(bytes, &mut offset)?;
        let pixel_count = read_u32(bytes, &mut offset)? as usize;

        let graph_id = read_utf8(bytes, &mut offset, graph_id_len, "graph_id")?;
        let node_id = read_utf8(bytes, &mut offset, node_id_len, "node_id")?;
        let name = read_utf8(bytes, &mut offset, name_len, "name")?;
        let layout_id = read_utf8(bytes, &mut offset, layout_id_len, "layout_id")?;
        let rgba = bytes
            .get(offset..)
            .ok_or_else(|| "binary frame rgba payload missing".to_owned())?
            .to_vec();
        if rgba.len() != pixel_count.saturating_mul(4) {
            return Err(format!(
                "binary frame rgba payload length {} does not match pixel_count {}",
                rgba.len(),
                pixel_count
            ));
        }

        Ok(Self {
            graph_id,
            node_id,
            name,
            layout: LedLayout {
                id: layout_id,
                pixel_count,
                width: if width == 0 {
                    None
                } else {
                    Some(width as usize)
                },
                height: if height == 0 {
                    None
                } else {
                    Some(height as usize)
                },
            },
            rgba,
        })
    }

    /// Converts the binary frame message back into the structured `ServerMessage` form used by the frontend.
    pub fn into_server_message(self) -> ServerMessage {
        let mut pixels = Vec::with_capacity(self.layout.pixel_count);
        for chunk in self.rgba.chunks_exact(4).take(self.layout.pixel_count) {
            pixels.push(crate::RgbaColor {
                r: chunk[0] as f32 / 255.0,
                g: chunk[1] as f32 / 255.0,
                b: chunk[2] as f32 / 255.0,
                a: chunk[3] as f32 / 255.0,
            });
        }

        ServerMessage::NodeRuntimeUpdate {
            graph_id: self.graph_id,
            node_id: self.node_id,
            values: vec![NodeRuntimeUpdateValue::Inline {
                name: self.name,
                value: InputValue::ColorFrame(crate::ColorFrame {
                    layout: self.layout,
                    pixels,
                }),
            }],
        }
    }
}

/// Appends a little-endian `u32` field to a binary frame payload.
fn push_u32(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(&(value as u32).to_le_bytes());
}

/// Reads a little-endian `u32` field from a binary frame payload and advances the cursor.
fn read_u32(bytes: &[u8], offset: &mut usize) -> Result<u32, String> {
    let end = offset.saturating_add(4);
    let chunk = bytes
        .get(*offset..end)
        .ok_or_else(|| "binary frame packet truncated while reading u32".to_owned())?;
    *offset = end;
    Ok(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
}

/// Reads a UTF-8 string field of the given byte length from a binary frame payload.
fn read_utf8(bytes: &[u8], offset: &mut usize, len: usize, field: &str) -> Result<String, String> {
    let end = offset.saturating_add(len);
    let chunk = bytes
        .get(*offset..end)
        .ok_or_else(|| format!("binary frame packet truncated while reading {field}"))?;
    *offset = end;
    String::from_utf8(chunk.to_vec())
        .map_err(|_| format!("binary frame packet {field} was not valid utf-8"))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// Describes a discovered WLED instance shown in the frontend and used by runtime nodes.
pub struct WledInstance {
    pub id: String,
    pub name: String,
    pub host: String,
    pub led_count: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::{BinaryRuntimeFrameMessage, NodeRuntimeUpdateValue, ServerMessage};
    use crate::{InputValue, LedLayout};

    /// Tests that binary frame messages round-trip without losing header or payload data.
    #[test]
    fn binary_runtime_frame_message_roundtrips() {
        let message = BinaryRuntimeFrameMessage {
            graph_id: "graph".to_owned(),
            node_id: "node".to_owned(),
            name: "frame".to_owned(),
            layout: LedLayout {
                id: "layout".to_owned(),
                pixel_count: 2,
                width: Some(2),
                height: Some(1),
            },
            rgba: vec![255, 128, 0, 255, 0, 64, 255, 128],
        };

        let decoded = BinaryRuntimeFrameMessage::decode(&message.encode())
            .expect("decode binary runtime frame packet");
        assert_eq!(decoded, message);
    }

    /// Tests that binary frame messages can be reconstructed into the structured server form.
    #[test]
    fn binary_runtime_frame_message_converts_into_server_message() {
        let message = BinaryRuntimeFrameMessage {
            graph_id: "graph".to_owned(),
            node_id: "node".to_owned(),
            name: "frame".to_owned(),
            layout: LedLayout {
                id: "layout".to_owned(),
                pixel_count: 1,
                width: Some(1),
                height: Some(1),
            },
            rgba: vec![255, 128, 0, 255],
        };

        let server_message = message.into_server_message();
        match server_message {
            ServerMessage::NodeRuntimeUpdate {
                graph_id,
                node_id,
                values,
            } => {
                assert_eq!(graph_id, "graph");
                assert_eq!(node_id, "node");
                assert_eq!(values.len(), 1);
                match &values[0] {
                    NodeRuntimeUpdateValue::Inline { name, value } => {
                        assert_eq!(name, "frame");
                        match value {
                            InputValue::ColorFrame(frame) => {
                                assert_eq!(frame.layout.pixel_count, 1);
                                assert_eq!(frame.pixels.len(), 1);
                            }
                            other => panic!("expected color frame, got {other:?}"),
                        }
                    }
                }
            }
            other => panic!("expected node runtime update, got {other:?}"),
        }
    }

    /// Tests that persisted broker configs without the new broker-intent field still default to
    /// Home Assistant-compatible behavior.
    #[test]
    fn mqtt_broker_config_defaults_to_home_assistant() {
        let json = r#"{
            "id": "broker",
            "display_name": "Broker",
            "host": "127.0.0.1",
            "port": 1883,
            "username": "",
            "password": "",
            "discovery_prefix": "homeassistant"
        }"#;

        let config: super::MqttBrokerConfig =
            serde_json::from_str(json).expect("deserialize broker config");
        assert!(config.is_home_assistant);
    }

    /// Tests that the broker-intent flag round-trips through serde without changing values.
    #[test]
    fn mqtt_broker_config_roundtrips_home_assistant_flag() {
        let config = super::MqttBrokerConfig {
            id: "broker".to_owned(),
            display_name: "Broker".to_owned(),
            host: "127.0.0.1".to_owned(),
            port: 1883,
            username: String::new(),
            password: String::new(),
            discovery_prefix: "homeassistant".to_owned(),
            is_home_assistant: false,
        };

        let encoded = serde_json::to_string(&config).expect("serialize broker config");
        let decoded: super::MqttBrokerConfig =
            serde_json::from_str(&encoded).expect("deserialize broker config");
        assert_eq!(decoded, config);
    }
}
