use eframe::egui;
use futures_channel::mpsc::TryRecvError;
use shared::ServerMessage;
use tracing::{debug, trace, warn};

use crate::app::FrontendApp;
use crate::controllers::autosave::canonicalize_graph_document;
use crate::controllers::subscriptions::format_subscriptions;

impl FrontendApp {
    /// Drains parsed server messages from the WebSocket receive queue and applies them in order.
    ///
    /// If the receive channel closes, the frontend treats that as a transport disconnect and
    /// schedules reconnection.
    pub(crate) fn drain_server_messages(&mut self, ctx: &egui::Context) {
        let Some(receiver) = &mut self.connection.incoming else {
            return;
        };

        let mut pending = Vec::new();
        let mut receiver_disconnected = false;
        loop {
            match receiver.try_recv() {
                Ok(message) => pending.push(message),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Closed) => {
                    receiver_disconnected = true;
                    break;
                }
            }
        }

        for message in pending {
            self.apply_server_message(ctx, message);
        }

        if receiver_disconnected {
            warn!("frontend incoming websocket channel disconnected");
            self.handle_connection_loss(ctx, "Disconnected".to_owned());
        }
    }

    /// Applies a single parsed server message to the frontend state model.
    fn apply_server_message(&mut self, ctx: &egui::Context, message: ServerMessage) {
        self.connection.has_confirmed_connection = true;
        match message {
            ServerMessage::Welcome { message } => self.ui.status = message,
            ServerMessage::State(server_state) => self.apply_server_state(server_state),
            ServerMessage::Pong => self.ui.status = "Received pong".to_owned(),
            ServerMessage::Error { message } => {
                warn!("frontend received server error");
                self.mark_graph_save_failed(Self::now_secs(ctx));
                self.ui.status = message;
            }
            ServerMessage::Event(event) => {
                trace!(topic = ?event.topic, scope = ?event.scope, "frontend applying event message");
                self.ui.last_event = format!("{}: {}", event.topic.label(), event.message);
            }
            ServerMessage::SubscriptionState { subscriptions } => {
                debug!("frontend updated event subscriptions");
                self.subscriptions.active_event_subscriptions =
                    subscriptions.iter().cloned().collect();
                self.subscriptions.subscriptions_status = format_subscriptions(&subscriptions);
                self.ui.status = "Subscription state updated".to_owned();
            }
            ServerMessage::GraphMetadata { documents } => {
                debug!("frontend received graph metadata update");
                self.apply_graph_metadata(documents);
            }
            ServerMessage::NodeDefinitions { definitions } => {
                debug!("frontend received node definitions update");
                self.apply_node_definitions(definitions);
            }
            ServerMessage::GraphDocument { document } => {
                debug!(graph_id = %document.metadata.id, "frontend received graph document");
                let canonical_document = canonicalize_graph_document(&document);
                let current_canonical = self
                    .graphs
                    .loaded_graph_document
                    .as_ref()
                    .map(canonicalize_graph_document);
                let save_acknowledged =
                    self.graphs.save_in_flight_document.as_ref() == Some(&canonical_document);

                if save_acknowledged {
                    self.acknowledge_graph_save(
                        document,
                        current_canonical.as_ref() == Some(&canonical_document),
                    );
                } else {
                    self.apply_graph_document_loaded(document);
                }
            }
            ServerMessage::GraphExport { file } => {
                debug!(
                    graph_id = %file.document.metadata.id,
                    "frontend received graph export"
                );
                self.handle_graph_export(file);
            }
            ServerMessage::GraphImported { document, mode } => {
                debug!(
                    graph_id = %document.metadata.id,
                    mode = ?mode,
                    "frontend received graph imported confirmation"
                );
                self.handle_graph_imported(document, mode);
            }
            ServerMessage::RuntimeStatuses { graphs } => {
                trace!("frontend received runtime statuses");
                self.apply_runtime_statuses(graphs);
            }
            ServerMessage::NodeRuntimeUpdate {
                graph_id,
                node_id,
                values,
            } => self.apply_runtime_update(graph_id, node_id, values),
            ServerMessage::GraphDiagnosticsSummary { graph_id, nodes } => {
                self.apply_graph_diagnostics_summary(graph_id, nodes);
            }
            ServerMessage::NodeDiagnosticsDetail {
                graph_id,
                node_id,
                diagnostics,
            } => self.apply_node_diagnostics_detail(graph_id, node_id, diagnostics),
            ServerMessage::WledInstances { instances } => {
                debug!("frontend received WLED instance update");
                self.apply_wled_instances(instances);
            }
            ServerMessage::MqttBrokerConfigs { brokers } => {
                debug!("frontend received MQTT broker config update");
                self.apply_mqtt_broker_configs(brokers);
            }
        }
    }
}
