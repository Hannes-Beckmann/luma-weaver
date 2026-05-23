use std::{collections::HashSet, sync::atomic::Ordering};

use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use futures_util::StreamExt;
use shared::{EventMessage, EventScope, EventTopic, ServerMessage};
use tokio::sync::broadcast;
use tracing::{debug, info, trace};

use crate::api::websocket::outbound::{
    current_state, disconnect_client, send_node_runtime_update, send_server_message,
};
use crate::api::websocket::routing::{handle_client_message, matches_any_subscription};
use crate::app::state::AppState;
use crate::messaging::event_bus::BackendEvent;

/// Upgrades an HTTP request into a backend WebSocket session.
pub(crate) async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    debug!("received websocket upgrade request");
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Runs the full backend WebSocket session loop for a connected client.
///
/// The session owns per-client subscription state, forwards inbound text messages through the
/// routing layer, and relays backend events and runtime updates until either side closes the
/// connection.
async fn handle_socket(socket: WebSocket, state: AppState) {
    let client_id = state.next_client_id.fetch_add(1, Ordering::SeqCst) + 1;
    let connected_clients = state.connected_clients.fetch_add(1, Ordering::SeqCst) + 1;
    let mut subscriptions = HashSet::new();
    let mut runtime_graph_subscriptions = HashSet::new();
    let mut diagnostics_graph_subscriptions = HashSet::new();
    let mut node_diagnostics_subscriptions = HashSet::new();
    let mut event_receiver = state.event_bus.subscribe();
    let (mut write, mut read) = socket.split();

    info!(client_id, connected_clients, "websocket connected");

    if send_server_message(
        &mut write,
        client_id,
        ServerMessage::Welcome {
            message: "Connected to the Luma Weaver backend".to_owned(),
        },
    )
    .await
    .is_err()
    {
        disconnect_client(&state, client_id);
        return;
    }

    if send_server_message(
        &mut write,
        client_id,
        ServerMessage::State(current_state(&state)),
    )
    .await
    .is_err()
    {
        disconnect_client(&state, client_id);
        return;
    }

    state.event_bus.emit_event(EventMessage {
        topic: EventTopic::Connection,
        scope: EventScope::Global,
        message: format!("Client {client_id} connected ({connected_clients} total)"),
    });

    loop {
        tokio::select! {
            message_result = read.next() => {
                let Some(message_result) = message_result else {
                    tracing::info!(client_id, "websocket stream closed by client");
                    break;
                };

                let message = match message_result {
                    Ok(message) => message,
                    Err(error) => {
                        tracing::warn!(client_id, %error, "websocket receive failed");
                        break;
                    }
                };

                let Message::Text(text) = message else {
                    tracing::debug!(client_id, kind = ?message, "ignoring non-text websocket message");
                    continue;
                };

                trace!(client_id, payload_bytes = text.len(), "received client websocket message");

                if handle_client_message(
                    &state,
                    &mut write,
                    client_id,
                    &mut subscriptions,
                    &mut runtime_graph_subscriptions,
                    &mut diagnostics_graph_subscriptions,
                    &mut node_diagnostics_subscriptions,
                    &text,
                )
                .await
                .is_err()
                {
                    break;
                }
            }
            event_result = event_receiver.recv() => {
                let event = match event_result {
                    Ok(event) => event,
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        tracing::debug!(
                            client_id,
                            skipped,
                            "websocket client lagged behind event bus"
                        );
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        tracing::warn!(client_id, "event bus closed");
                        break;
                    }
                };

                match event {
                    BackendEvent::EventMessage(event) => {
                        if !matches_any_subscription(&subscriptions, &event.topic, &event.scope) {
                            continue;
                        }
                        if send_server_message(&mut write, client_id, ServerMessage::Event(event))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    BackendEvent::GraphMetadataChanged { documents } => {
                        if !matches_any_subscription(
                            &subscriptions,
                            &EventTopic::GraphMetadataChanged,
                            &EventScope::Global,
                        ) {
                            continue;
                        }
                        if send_server_message(
                            &mut write,
                            client_id,
                            ServerMessage::GraphMetadata { documents },
                        )
                        .await
                        .is_err()
                        {
                            break;
                        }
                    }
                    BackendEvent::RuntimeStatusesChanged { statuses } => {
                        if send_server_message(
                            &mut write,
                            client_id,
                            ServerMessage::RuntimeStatuses { graphs: statuses },
                        )
                        .await
                        .is_err()
                        {
                            break;
                        }
                    }
                    BackendEvent::NodeRuntimeUpdate {
                        graph_id,
                        node_id,
                        values,
                    } => {
                        if !runtime_graph_subscriptions.contains(&graph_id) {
                            continue;
                        }
                        if send_node_runtime_update(
                            &mut write,
                            client_id,
                            graph_id,
                            node_id,
                            values,
                        )
                        .await
                        .is_err()
                        {
                            break;
                        }
                    }
                    BackendEvent::GraphDiagnosticsSummaryChanged { graph_id, nodes } => {
                        if !diagnostics_graph_subscriptions.contains(&graph_id) {
                            continue;
                        }
                        if send_server_message(
                            &mut write,
                            client_id,
                            ServerMessage::GraphDiagnosticsSummary { graph_id, nodes },
                        )
                        .await
                        .is_err()
                        {
                            break;
                        }
                    }
                    BackendEvent::NodeDiagnosticsDetailChanged {
                        graph_id,
                        node_id,
                        diagnostics,
                    } => {
                        if !node_diagnostics_subscriptions
                            .contains(&(graph_id.clone(), node_id.clone()))
                        {
                            continue;
                        }
                        if send_server_message(
                            &mut write,
                            client_id,
                            ServerMessage::NodeDiagnosticsDetail {
                                graph_id,
                                node_id,
                                diagnostics,
                            },
                        )
                        .await
                        .is_err()
                        {
                            break;
                        }
                    }
                    BackendEvent::WledInstancesChanged { instances } => {
                        if send_server_message(
                            &mut write,
                            client_id,
                            ServerMessage::WledInstances { instances },
                        )
                        .await
                        .is_err()
                        {
                            break;
                        }
                    }
                }
            }
        }
    }

    disconnect_client(&state, client_id);
}
