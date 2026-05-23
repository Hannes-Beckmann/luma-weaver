use eframe::egui;
use futures_channel::mpsc;
use shared::{ClientMessage, ServerMessage};

pub(crate) struct FrontendTransport {
    kind: TransportKind,
    pub(crate) sender: mpsc::UnboundedSender<ClientMessage>,
    pub(crate) incoming: mpsc::UnboundedReceiver<ServerMessage>,
    #[cfg(target_arch = "wasm32")]
    implementation: TransportImplementation,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum TransportKind {
    Websocket,
    Demo,
}

#[cfg(target_arch = "wasm32")]
impl FrontendTransport {
    pub(crate) fn connect(ctx: &egui::Context) -> Result<(Self, String), String> {
        #[cfg(feature = "demo-mode")]
        {
            let _ = ctx;
            let (sender, incoming, demo_service) = backend::demo::connect_demo()
                .map_err(|error| format!("Failed to start demo transport: {error}"))?;
            return Ok((
                Self {
                    kind: TransportKind::Demo,
                    sender,
                    incoming,
                    implementation: TransportImplementation::Demo { demo_service },
                },
                "Demo".to_owned(),
            ));
        }

        #[cfg(not(feature = "demo-mode"))]
        {
            return websocket::connect(ctx);
        }
    }

    pub(crate) fn kind(&self) -> TransportKind {
        self.kind
    }

    pub(crate) fn poll(&mut self, now_secs: f64) -> Option<String> {
        self.implementation.poll(now_secs)
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl FrontendTransport {
    pub(crate) fn connect(ctx: &egui::Context) -> Result<(Self, String), String> {
        let _ = ctx;
        Err("Frontend transport is only available in wasm builds".to_owned())
    }

    pub(crate) fn kind(&self) -> TransportKind {
        self.kind
    }

    pub(crate) fn poll(&mut self, now_secs: f64) -> Option<String> {
        let _ = now_secs;
        None
    }
}

#[cfg(target_arch = "wasm32")]
enum TransportImplementation {
    Websocket {
        events: mpsc::UnboundedReceiver<websocket::WebSocketEvent>,
    },
    #[cfg(feature = "demo-mode")]
    Demo {
        demo_service: backend::demo::DemoTransport,
    },
}

#[cfg(target_arch = "wasm32")]
impl TransportImplementation {
    fn poll(&mut self, now_secs: f64) -> Option<String> {
        match self {
            Self::Websocket { events } => {
                let mut disconnected = None;
                while let Ok(event) = events.try_recv() {
                    match event {
                        websocket::WebSocketEvent::Disconnected { reason } => {
                            disconnected = Some(reason);
                        }
                    }
                }
                disconnected
            }
            #[cfg(feature = "demo-mode")]
            Self::Demo { demo_service } => {
                demo_service.pump(now_secs);
                None
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod websocket {
    use eframe::egui;
    use futures_channel::mpsc;
    use futures_util::{SinkExt, StreamExt};
    use gloo_net::websocket::{Message, futures::WebSocket};
    use shared::{BinaryRuntimeFrameMessage, ClientMessage, ServerMessage};
    use tracing::{error, info, trace, warn};

    use super::{FrontendTransport, TransportImplementation, TransportKind};

    pub(super) enum WebSocketEvent {
        Disconnected { reason: String },
    }

    pub(super) fn connect(ctx: &egui::Context) -> Result<(FrontendTransport, String), String> {
        let url = websocket_url();
        info!("frontend opening websocket connection");

        match WebSocket::open(&url) {
            Ok(socket) => {
                info!("frontend websocket connected");
                let (mut write, mut read) = socket.split();
                let (sender, mut outgoing_receiver) = mpsc::unbounded::<ClientMessage>();
                let (incoming_sender, incoming) = mpsc::unbounded::<ServerMessage>();
                let (event_sender, events) = mpsc::unbounded::<WebSocketEvent>();

                let repaint_ctx = ctx.clone();
                let outgoing_events = event_sender.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    while let Some(message) = outgoing_receiver.next().await {
                        trace!(
                            kind = client_message_kind(&message),
                            "frontend sending websocket message"
                        );
                        let serialized = match serde_json::to_string(&message) {
                            Ok(serialized) => serialized,
                            Err(error) => {
                                error!(%error, kind = client_message_kind(&message), "frontend failed to serialize client message");
                                continue;
                            }
                        };

                        if let Err(error) = write.send(Message::Text(serialized)).await {
                            error!(%error, "frontend websocket send failed");
                            let _ = outgoing_events.unbounded_send(WebSocketEvent::Disconnected {
                                reason: format!("send failed: {error}"),
                            });
                            break;
                        }

                        repaint_ctx.request_repaint();
                    }

                    trace!("frontend outgoing websocket task stopped");
                });

                let repaint_ctx = ctx.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    while let Some(message) = read.next().await {
                        let message = match message {
                            Ok(message) => message,
                            Err(error) => {
                                error!(%error, "frontend websocket receive failed");
                                let _ = event_sender.unbounded_send(WebSocketEvent::Disconnected {
                                    reason: format!("receive failed: {error}"),
                                });
                                break;
                            }
                        };

                        match message {
                            Message::Text(text) => {
                                if let Ok(server_message) =
                                    serde_json::from_str::<ServerMessage>(&text)
                                {
                                    trace!(
                                        kind = server_message_kind(&server_message),
                                        payload_bytes = text.len(),
                                        "frontend received websocket message"
                                    );
                                    let _ = incoming_sender.unbounded_send(server_message);
                                } else {
                                    warn!(
                                        payload_bytes = text.len(),
                                        "frontend failed to parse server message"
                                    );
                                }
                            }
                            Message::Bytes(bytes) => {
                                match BinaryRuntimeFrameMessage::decode(&bytes)
                                    .map(BinaryRuntimeFrameMessage::into_server_message)
                                {
                                    Ok(server_message) => {
                                        trace!(
                                            kind = server_message_kind(&server_message),
                                            payload_bytes = bytes.len(),
                                            "frontend received binary websocket message"
                                        );
                                        let _ = incoming_sender.unbounded_send(server_message);
                                    }
                                    Err(error) => {
                                        warn!(%error, payload_bytes = bytes.len(), "frontend failed to parse binary frame message");
                                    }
                                }
                            }
                        }

                        repaint_ctx.request_repaint();
                    }

                    let _ = event_sender.unbounded_send(WebSocketEvent::Disconnected {
                        reason: "stream closed".to_owned(),
                    });
                    warn!("frontend websocket stream closed");
                });

                Ok((
                    FrontendTransport {
                        kind: TransportKind::Websocket,
                        sender,
                        incoming,
                        implementation: TransportImplementation::Websocket { events },
                    },
                    format!("Connected to {url}"),
                ))
            }
            Err(error) => {
                error!(%error, "frontend failed to connect websocket");
                Err(format!("Failed to connect to {url}: {error}"))
            }
        }
    }

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
            ClientMessage::UpdateGraphExecutionFrequency { .. } => {
                "update_graph_execution_frequency"
            }
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

    fn websocket_url() -> String {
        let Some(window) = web_sys::window() else {
            let fallback = "ws://127.0.0.1:3000/ws".to_owned();
            warn!("frontend window unavailable, using fallback websocket url");
            return fallback;
        };

        let location = window.location();
        let protocol = match location.protocol().ok().as_deref() {
            Some("https:") => "wss",
            _ => "ws",
        };
        let host = location
            .host()
            .ok()
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "127.0.0.1:3000".to_owned());

        format!("{protocol}://{host}/ws")
    }
}
