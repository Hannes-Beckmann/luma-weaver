//! Shared event-bus message and subscription types.
//!
//! These types are intentionally lighter-weight than the main protocol in `protocol.rs` and are
//! used for status-style notifications and subscription filtering.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// Represents the coarse-grained backend status snapshot sent alongside other frontend updates.
///
/// This is intentionally small and is used to answer questions like "is the backend alive?" and
/// "how many clients are currently connected?".
pub struct ServerState {
    pub connected_clients: usize,
    pub status: String,
}

impl Default for ServerState {
    /// Builds the default server-state snapshot used before any live status is known.
    fn default() -> Self {
        Self {
            connected_clients: 0,
            status: "ready".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
/// Classifies an event so clients can subscribe to broad categories of backend activity.
pub enum EventTopic {
    Connection,
    Ping,
    Name,
    GraphMetadataChanged,
}

impl EventTopic {
    /// Lists every currently defined event topic.
    pub const ALL: [Self; 4] = [
        Self::Connection,
        Self::Ping,
        Self::Name,
        Self::GraphMetadataChanged,
    ];

    /// Returns a human-readable label for the topic.
    pub fn label(self) -> &'static str {
        match self {
            Self::Connection => "Connection",
            Self::Ping => "Ping",
            Self::Name => "Name",
            Self::GraphMetadataChanged => "GraphMetadataChanged",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// Represents one event-bus message emitted by the backend.
///
/// Event messages are lightweight status or audit-style notifications, distinct from the heavier
/// structured request/response protocol in `protocol.rs`.
pub struct EventMessage {
    pub topic: EventTopic,
    pub scope: EventScope,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
/// Describes the scope to which an event or subscription applies.
///
/// Broader scopes are allowed to match narrower emitted events, for example a graph subscription
/// receiving node-scoped events inside the same graph.
pub enum EventScope {
    Global,
    Graph {
        graph_id: String,
    },
    Node {
        graph_id: String,
        node_id: String,
    },
    Element {
        graph_id: String,
        node_id: String,
        element_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
/// Subscribes to one event topic within one scope.
pub struct EventSubscription {
    pub topic: EventTopic,
    pub scope: EventScope,
}
