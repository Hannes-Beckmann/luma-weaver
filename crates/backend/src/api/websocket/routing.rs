mod diagnostics;
mod graphs;
mod integrations;
mod runtime;
mod session;

use std::collections::HashSet;

use axum::extract::ws::{Message, WebSocket};
use futures_util::stream::SplitSink;
use shared::{ClientMessage, EventScope, EventSubscription, EventTopic, ServerMessage};

use crate::api::websocket::outbound::{current_state, send_server_message};
use crate::app::state::AppState;

pub(crate) struct RoutingContext<'a> {
    pub(crate) state: &'a AppState,
    pub(crate) client_id: usize,
    pub(crate) subscriptions: &'a mut HashSet<EventSubscription>,
    pub(crate) runtime_graph_subscriptions: &'a mut HashSet<String>,
    pub(crate) diagnostics_graph_subscriptions: &'a mut HashSet<String>,
    pub(crate) node_diagnostics_subscriptions: &'a mut HashSet<(String, String)>,
}

/// Parses and routes a single client WebSocket message, then sends the refreshed server snapshot.
///
/// Domain-specific handlers may optionally return a direct response message. Regardless of whether
/// a direct response is produced, the current aggregate server state is pushed afterward so the
/// client can reconcile derived UI state from a single source of truth.
pub(crate) async fn handle_client_message(
    state: &AppState,
    write: &mut SplitSink<WebSocket, Message>,
    client_id: usize,
    subscriptions: &mut HashSet<EventSubscription>,
    runtime_graph_subscriptions: &mut HashSet<String>,
    diagnostics_graph_subscriptions: &mut HashSet<String>,
    node_diagnostics_subscriptions: &mut HashSet<(String, String)>,
    text: &str,
) -> Result<(), axum::Error> {
    let response = match serde_json::from_str::<ClientMessage>(text) {
        Ok(client_message) => {
            tracing::trace!(
                client_id,
                kind = client_message_kind(&client_message),
                "parsed client message"
            );

            let mut context = RoutingContext {
                state,
                client_id,
                subscriptions,
                runtime_graph_subscriptions,
                diagnostics_graph_subscriptions,
                node_diagnostics_subscriptions,
            };

            match client_message {
                ClientMessage::Ping
                | ClientMessage::SetName { .. }
                | ClientMessage::Subscribe { .. }
                | ClientMessage::Unsubscribe { .. } => {
                    session::handle(&mut context, client_message).await
                }
                ClientMessage::CreateGraphDocument { .. }
                | ClientMessage::DeleteGraphDocument { .. }
                | ClientMessage::GetGraphDocument { .. }
                | ClientMessage::ExportGraphDocument { .. }
                | ClientMessage::UpdateGraphDocument { .. }
                | ClientMessage::UpdateGraphName { .. }
                | ClientMessage::ImportGraphDocument { .. }
                | ClientMessage::UpdateGraphExecutionFrequency { .. }
                | ClientMessage::GetNodeDefinitions
                | ClientMessage::GetGraphMetadata => {
                    graphs::handle(&mut context, client_message).await
                }
                ClientMessage::StartGraph { .. }
                | ClientMessage::PauseGraph { .. }
                | ClientMessage::StepGraph { .. }
                | ClientMessage::StopGraph { .. }
                | ClientMessage::GetRuntimeStatuses
                | ClientMessage::SubscribeGraphRuntime { .. }
                | ClientMessage::UnsubscribeGraphRuntime { .. } => {
                    runtime::handle(&mut context, client_message).await
                }
                ClientMessage::SubscribeGraphDiagnostics { .. }
                | ClientMessage::UnsubscribeGraphDiagnostics { .. }
                | ClientMessage::SubscribeNodeDiagnostics { .. }
                | ClientMessage::UnsubscribeNodeDiagnostics { .. }
                | ClientMessage::ClearNodeDiagnostics { .. } => {
                    diagnostics::handle(&mut context, client_message).await
                }
                ClientMessage::GetWledInstances
                | ClientMessage::GetMqttBrokerConfigs
                | ClientMessage::UpdateMqttBrokerConfigs { .. } => {
                    integrations::handle(&mut context, client_message).await
                }
            }
        }
        Err(error) => {
            tracing::warn!(client_id, %error, payload_bytes = text.len(), "failed to parse client message");
            Some(ServerMessage::Error {
                message: format!("Invalid client message: {error}"),
            })
        }
    };

    if let Some(response) = response {
        send_server_message(write, client_id, response).await?;
    }

    send_server_message(write, client_id, ServerMessage::State(current_state(state))).await?;
    Ok(())
}

/// Returns a stable logging key for a parsed client message variant.
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

/// Returns subscriptions in a deterministic order for logging, testing, and snapshot comparison.
pub(crate) fn sorted_subscriptions(
    subscriptions: &HashSet<EventSubscription>,
) -> Vec<EventSubscription> {
    let mut sorted = subscriptions.iter().cloned().collect::<Vec<_>>();
    sorted.sort_by_key(subscription_sort_key);
    sorted
}

/// Builds a sortable key that groups subscriptions by topic and then by narrowing scope.
fn subscription_sort_key(subscription: &EventSubscription) -> (u8, u8, String, String, String) {
    let topic_key = match subscription.topic {
        EventTopic::Connection => 0,
        EventTopic::Ping => 1,
        EventTopic::Name => 2,
        EventTopic::GraphMetadataChanged => 3,
    };

    match &subscription.scope {
        EventScope::Global => (topic_key, 0, String::new(), String::new(), String::new()),
        EventScope::Graph { graph_id } => {
            (topic_key, 1, graph_id.clone(), String::new(), String::new())
        }
        EventScope::Node { graph_id, node_id } => (
            topic_key,
            2,
            graph_id.clone(),
            node_id.clone(),
            String::new(),
        ),
        EventScope::Element {
            graph_id,
            node_id,
            element_id,
        } => (
            topic_key,
            3,
            graph_id.clone(),
            node_id.clone(),
            element_id.clone(),
        ),
    }
}

/// Returns whether any subscription in the set should receive an event at the given topic and scope.
pub(crate) fn matches_any_subscription(
    subscriptions: &HashSet<EventSubscription>,
    topic: &EventTopic,
    event_scope: &EventScope,
) -> bool {
    subscriptions.iter().any(|subscription| {
        subscription.topic == *topic && scope_matches(&subscription.scope, event_scope)
    })
}

/// Returns whether an emitted event scope is covered by a subscription scope.
///
/// Broader scopes intentionally match narrower emitted events, such as a graph subscription
/// receiving node- and element-scoped events inside the same graph.
fn scope_matches(subscription_scope: &EventScope, event_scope: &EventScope) -> bool {
    match (subscription_scope, event_scope) {
        (EventScope::Global, _) => true,
        (EventScope::Graph { graph_id: lhs }, EventScope::Graph { graph_id: rhs }) => lhs == rhs,
        (EventScope::Graph { graph_id: lhs }, EventScope::Node { graph_id: rhs, .. }) => lhs == rhs,
        (EventScope::Graph { graph_id: lhs }, EventScope::Element { graph_id: rhs, .. }) => {
            lhs == rhs
        }
        (
            EventScope::Node {
                graph_id: lhs_graph,
                node_id: lhs_node,
            },
            EventScope::Node {
                graph_id: rhs_graph,
                node_id: rhs_node,
            },
        ) => lhs_graph == rhs_graph && lhs_node == rhs_node,
        (
            EventScope::Node {
                graph_id: lhs_graph,
                node_id: lhs_node,
            },
            EventScope::Element {
                graph_id: rhs_graph,
                node_id: rhs_node,
                ..
            },
        ) => lhs_graph == rhs_graph && lhs_node == rhs_node,
        (
            EventScope::Element {
                graph_id: lhs_graph,
                node_id: lhs_node,
                element_id: lhs_element,
            },
            EventScope::Element {
                graph_id: rhs_graph,
                node_id: rhs_node,
                element_id: rhs_element,
            },
        ) => lhs_graph == rhs_graph && lhs_node == rhs_node && lhs_element == rhs_element,
        _ => false,
    }
}
