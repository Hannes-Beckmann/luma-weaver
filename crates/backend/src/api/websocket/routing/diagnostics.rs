use shared::{ClientMessage, ServerMessage};

use super::RoutingContext;

/// Handles graph- and node-diagnostics subscription messages for a single client request.
///
/// New subscriptions immediately return the current diagnostic snapshot so the client does not
/// need to wait for the next event-bus update before populating the UI.
pub(super) async fn handle(
    context: &mut RoutingContext<'_>,
    message: ClientMessage,
) -> Option<ServerMessage> {
    match message {
        ClientMessage::SubscribeGraphDiagnostics { graph_id } if graph_id.trim().is_empty() => {
            Some(ServerMessage::Error {
                message: "Graph document id must not be empty".to_owned(),
            })
        }
        ClientMessage::SubscribeGraphDiagnostics { graph_id } => {
            let graph_id = graph_id.trim().to_owned();
            context
                .diagnostics_graph_subscriptions
                .insert(graph_id.clone());
            Some(ServerMessage::GraphDiagnosticsSummary {
                graph_id: graph_id.clone(),
                nodes: context.state.event_bus.graph_diagnostics_summary(&graph_id),
            })
        }
        ClientMessage::UnsubscribeGraphDiagnostics { graph_id } if graph_id.trim().is_empty() => {
            Some(ServerMessage::Error {
                message: "Graph document id must not be empty".to_owned(),
            })
        }
        ClientMessage::UnsubscribeGraphDiagnostics { graph_id } => {
            context
                .diagnostics_graph_subscriptions
                .remove(graph_id.trim());
            None
        }
        ClientMessage::SubscribeNodeDiagnostics { graph_id, node_id }
            if graph_id.trim().is_empty() || node_id.trim().is_empty() =>
        {
            Some(ServerMessage::Error {
                message: "Graph document id and node id must not be empty".to_owned(),
            })
        }
        ClientMessage::SubscribeNodeDiagnostics { graph_id, node_id } => {
            let graph_id = graph_id.trim().to_owned();
            let node_id = node_id.trim().to_owned();
            context
                .node_diagnostics_subscriptions
                .insert((graph_id.clone(), node_id.clone()));
            Some(ServerMessage::NodeDiagnosticsDetail {
                graph_id: graph_id.clone(),
                node_id: node_id.clone(),
                diagnostics: context
                    .state
                    .event_bus
                    .node_diagnostics_detail(&graph_id, &node_id),
            })
        }
        ClientMessage::UnsubscribeNodeDiagnostics { graph_id, node_id }
            if graph_id.trim().is_empty() || node_id.trim().is_empty() =>
        {
            Some(ServerMessage::Error {
                message: "Graph document id and node id must not be empty".to_owned(),
            })
        }
        ClientMessage::UnsubscribeNodeDiagnostics { graph_id, node_id } => {
            context
                .node_diagnostics_subscriptions
                .remove(&(graph_id.trim().to_owned(), node_id.trim().to_owned()));
            None
        }
        ClientMessage::ClearNodeDiagnostics { graph_id, node_id }
            if graph_id.trim().is_empty() || node_id.trim().is_empty() =>
        {
            Some(ServerMessage::Error {
                message: "Graph document id and node id must not be empty".to_owned(),
            })
        }
        ClientMessage::ClearNodeDiagnostics { graph_id, node_id } => {
            context
                .state
                .event_bus
                .clear_node_diagnostics(graph_id.trim(), node_id.trim());
            Some(ServerMessage::NodeDiagnosticsDetail {
                graph_id: graph_id.trim().to_owned(),
                node_id: node_id.trim().to_owned(),
                diagnostics: Vec::new(),
            })
        }
        _ => unreachable!("diagnostics handler received unsupported message"),
    }
}
