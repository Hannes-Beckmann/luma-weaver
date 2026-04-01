use shared::ClientMessage;
use tracing::{error, trace, warn};

use super::FrontendApp;

impl FrontendApp {
    /// Queues a client message for the WebSocket task if a connection is currently available.
    ///
    /// When no connection exists, or the send queue rejects the message, the user-visible status
    /// text is updated so the failed action is not silent.
    pub(crate) fn send(&mut self, message: ClientMessage) {
        match &self.connection.sender {
            Some(sender) => {
                trace!(
                    kind = client_message_kind(&message),
                    "frontend queueing client message"
                );
                if let Err(error) = sender.unbounded_send(message) {
                    error!(%error, "frontend failed to queue client message");
                    self.ui.status = "Failed to queue message for WebSocket".to_owned();
                }
            }
            None => {
                warn!("frontend send requested while websocket is disconnected");
                self.ui.status = "WebSocket is not connected".to_owned();
            }
        }
    }
}

/// Returns a stable logging key for an outbound client message variant.
fn client_message_kind(message: &ClientMessage) -> &'static str {
    match message {
        ClientMessage::Ping => "ping",
        ClientMessage::SetName { .. } => "set_name",
        ClientMessage::Subscribe { .. } => "subscribe",
        ClientMessage::Unsubscribe { .. } => "unsubscribe",
        ClientMessage::CreateGraphDocument { .. } => "create_graph_document",
        ClientMessage::DeleteGraphDocument { .. } => "delete_graph_document",
        ClientMessage::GetGraphDocument { .. } => "get_graph_document",
        ClientMessage::ExportGraphDocument { .. } => "export_graph_document",
        ClientMessage::UpdateGraphDocument { .. } => "update_graph_document",
        ClientMessage::UpdateGraphName { .. } => "update_graph_name",
        ClientMessage::ImportGraphDocument { .. } => "import_graph_document",
        ClientMessage::UpdateGraphExecutionFrequency { .. } => "update_graph_execution_frequency",
        ClientMessage::GetNodeDefinitions => "get_node_definitions",
        ClientMessage::GetGraphMetadata => "get_graph_metadata",
        ClientMessage::StartGraph { .. } => "start_graph",
        ClientMessage::PauseGraph { .. } => "pause_graph",
        ClientMessage::StepGraph { .. } => "step_graph",
        ClientMessage::StopGraph { .. } => "stop_graph",
        ClientMessage::GetRuntimeStatuses => "get_runtime_statuses",
        ClientMessage::SubscribeGraphRuntime { .. } => "subscribe_graph_runtime",
        ClientMessage::UnsubscribeGraphRuntime { .. } => "unsubscribe_graph_runtime",
        ClientMessage::SubscribeGraphDiagnostics { .. } => "subscribe_graph_diagnostics",
        ClientMessage::UnsubscribeGraphDiagnostics { .. } => "unsubscribe_graph_diagnostics",
        ClientMessage::SubscribeNodeDiagnostics { .. } => "subscribe_node_diagnostics",
        ClientMessage::UnsubscribeNodeDiagnostics { .. } => "unsubscribe_node_diagnostics",
        ClientMessage::ClearNodeDiagnostics { .. } => "clear_node_diagnostics",
        ClientMessage::GetWledInstances => "get_wled_instances",
        ClientMessage::GetMqttBrokerConfigs => "get_mqtt_broker_configs",
        ClientMessage::UpdateMqttBrokerConfigs { .. } => "update_mqtt_broker_configs",
    }
}
