use std::sync::atomic::Ordering;

use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, stream::SplitSink};
use shared::{
    BinaryRuntimeFrameMessage, EventMessage, EventScope, EventTopic, InputValue,
    NodeRuntimeUpdateValue, NodeRuntimeValue, ServerMessage, ServerState,
};
use tracing::{info, trace};

use crate::app::state::AppState;

/// Builds the aggregate server-state snapshot sent after each handled client message.
pub(crate) fn current_state(state: &AppState) -> ServerState {
    ServerState {
        connected_clients: state.connected_clients.load(Ordering::SeqCst),
        status: "running".to_owned(),
    }
}

/// Serializes and sends a structured server message over the WebSocket connection.
pub(crate) async fn send_server_message(
    write: &mut SplitSink<WebSocket, Message>,
    client_id: usize,
    message: ServerMessage,
) -> Result<(), axum::Error> {
    let text = serde_json::to_string(&message).expect("server messages must serialize");
    trace!(
        client_id,
        kind = server_message_kind(&message),
        payload_bytes = text.len(),
        "sending server websocket message"
    );
    write.send(Message::Text(text.into())).await
}

/// Sends a node runtime update, using binary transport for frame payloads when possible.
///
/// `ColorFrame` values named `frame` are emitted as binary RGBA payloads to avoid bloating the
/// JSON channel. Remaining values are sent through the structured `NodeRuntimeUpdate` message.
pub(crate) async fn send_node_runtime_update(
    write: &mut SplitSink<WebSocket, Message>,
    client_id: usize,
    graph_id: String,
    node_id: String,
    values: Vec<NodeRuntimeValue>,
) -> Result<(), axum::Error> {
    let mut inline_values = Vec::new();

    for value in values {
        let NodeRuntimeValue { name, value } = value;
        match value {
            InputValue::ColorFrame(frame) if name == "frame" => {
                let binary_message = BinaryRuntimeFrameMessage {
                    graph_id: graph_id.clone(),
                    node_id: node_id.clone(),
                    name,
                    layout: frame.layout,
                    rgba: encode_rgba_bytes(&frame.pixels),
                };
                let payload = binary_message.encode();
                trace!(
                    client_id,
                    graph_id,
                    node_id,
                    payload_bytes = payload.len(),
                    "sending binary frame runtime update"
                );
                write.send(Message::Binary(payload.into())).await?;
            }
            value => inline_values.push(NodeRuntimeUpdateValue::Inline { name, value }),
        }
    }

    if inline_values.is_empty() {
        return Ok(());
    }

    send_server_message(
        write,
        client_id,
        ServerMessage::NodeRuntimeUpdate {
            graph_id,
            node_id,
            values: inline_values,
        },
    )
    .await
}

/// Updates connection bookkeeping and emits the global disconnect event for a client.
pub(crate) fn disconnect_client(state: &AppState, client_id: usize) {
    let connected_clients = state.connected_clients.fetch_sub(1, Ordering::SeqCst) - 1;
    state.event_bus.emit_event(EventMessage {
        topic: EventTopic::Connection,
        scope: EventScope::Global,
        message: format!("Client {client_id} disconnected ({connected_clients} total)"),
    });
    info!(client_id, connected_clients, "websocket disconnected");
}

/// Returns a stable logging key for a structured server message variant.
fn server_message_kind(message: &ServerMessage) -> &'static str {
    match message {
        ServerMessage::Welcome { .. } => "welcome",
        ServerMessage::State(_) => "state",
        ServerMessage::Pong => "pong",
        ServerMessage::Error { .. } => "error",
        ServerMessage::Event(_) => "event",
        ServerMessage::SubscriptionState { .. } => "subscription_state",
        ServerMessage::GraphMetadata { .. } => "graph_metadata",
        ServerMessage::NodeDefinitions { .. } => "node_definitions",
        ServerMessage::GraphDocument { .. } => "graph_document",
        ServerMessage::GraphExport { .. } => "graph_export",
        ServerMessage::GraphImported { .. } => "graph_imported",
        ServerMessage::RuntimeStatuses { .. } => "runtime_statuses",
        ServerMessage::NodeRuntimeUpdate { .. } => "node_runtime_update",
        ServerMessage::GraphDiagnosticsSummary { .. } => "graph_diagnostics_summary",
        ServerMessage::NodeDiagnosticsDetail { .. } => "node_diagnostics_detail",
        ServerMessage::WledInstances { .. } => "wled_instances",
        ServerMessage::MqttBrokerConfigs { .. } => "mqtt_broker_configs",
    }
}

/// Encodes normalized RGBA pixels into packed 8-bit channel bytes for binary frame transport.
pub(crate) fn encode_rgba_bytes(pixels: &[shared::RgbaColor]) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(pixels.len() * 4);
    for pixel in pixels {
        rgba.push(unit_float_to_u8(pixel.r));
        rgba.push(unit_float_to_u8(pixel.g));
        rgba.push(unit_float_to_u8(pixel.b));
        rgba.push(unit_float_to_u8(pixel.a));
    }
    rgba
}

/// Converts a normalized float color channel into an 8-bit integer channel.
fn unit_float_to_u8(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use shared::{ColorFrame, RgbaColor};

    use super::encode_rgba_bytes;

    /// Tests that frame pixels are encoded as packed RGBA bytes in source order.
    #[test]
    fn frame_runtime_updates_are_encoded_as_rgba_bytes() {
        let frame = ColorFrame {
            layout: shared::LedLayout {
                id: "frame".to_owned(),
                pixel_count: 2,
                width: Some(2),
                height: Some(1),
            },
            pixels: vec![
                RgbaColor {
                    r: 1.0,
                    g: 0.5,
                    b: 0.0,
                    a: 1.0,
                },
                RgbaColor {
                    r: 0.0,
                    g: 0.25,
                    b: 1.0,
                    a: 0.5,
                },
            ],
        };

        assert_eq!(
            encode_rgba_bytes(&frame.pixels),
            vec![255, 128, 0, 255, 0, 64, 255, 128]
        );
    }
}
