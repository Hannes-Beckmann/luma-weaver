use shared::{ClientMessage, ServerMessage};

use super::RoutingContext;

/// Handles runtime-control and runtime-subscription messages for a single client request.
///
/// Direct control messages return the refreshed runtime status snapshot produced by the runtime
/// manager, while subscribe and unsubscribe messages only mutate the per-connection stream
/// bookkeeping.
pub(super) async fn handle(
    context: &mut RoutingContext<'_>,
    message: ClientMessage,
) -> Option<ServerMessage> {
    match message {
        ClientMessage::StartGraph { id } if id.trim().is_empty() => Some(ServerMessage::Error {
            message: "Graph document id must not be empty".to_owned(),
        }),
        ClientMessage::StartGraph { id } => {
            let id = id.trim().to_owned();
            match context.state.runtime_manager.start_graph(&id).await {
                Ok(update) => Some(ServerMessage::RuntimeStatuses {
                    graphs: update.statuses,
                }),
                Err(error) => Some(ServerMessage::Error {
                    message: error.to_string(),
                }),
            }
        }
        ClientMessage::PauseGraph { id } if id.trim().is_empty() => Some(ServerMessage::Error {
            message: "Graph document id must not be empty".to_owned(),
        }),
        ClientMessage::PauseGraph { id } => {
            let id = id.trim().to_owned();
            match context.state.runtime_manager.pause_graph(&id).await {
                Ok(update) => Some(ServerMessage::RuntimeStatuses {
                    graphs: update.statuses,
                }),
                Err(error) => Some(ServerMessage::Error {
                    message: error.to_string(),
                }),
            }
        }
        ClientMessage::StepGraph { id, ticks: _ } if id.trim().is_empty() => {
            Some(ServerMessage::Error {
                message: "Graph document id must not be empty".to_owned(),
            })
        }
        ClientMessage::StepGraph { id: _, ticks } if ticks == 0 => Some(ServerMessage::Error {
            message: "Tick count must be greater than zero".to_owned(),
        }),
        ClientMessage::StepGraph { id, ticks } => {
            let id = id.trim().to_owned();
            match context
                .state
                .runtime_manager
                .step_graph(&id, ticks as usize)
                .await
            {
                Ok(update) => Some(ServerMessage::RuntimeStatuses {
                    graphs: update.statuses,
                }),
                Err(error) => Some(ServerMessage::Error {
                    message: error.to_string(),
                }),
            }
        }
        ClientMessage::StopGraph { id } if id.trim().is_empty() => Some(ServerMessage::Error {
            message: "Graph document id must not be empty".to_owned(),
        }),
        ClientMessage::StopGraph { id } => {
            let id = id.trim().to_owned();
            match context.state.runtime_manager.stop_graph(&id).await {
                Ok(update) => Some(ServerMessage::RuntimeStatuses {
                    graphs: update.statuses,
                }),
                Err(error) => Some(ServerMessage::Error {
                    message: error.to_string(),
                }),
            }
        }
        ClientMessage::GetRuntimeStatuses => {
            let graphs = context.state.runtime_manager.runtime_statuses().await;
            Some(ServerMessage::RuntimeStatuses { graphs })
        }
        ClientMessage::SubscribeGraphRuntime { graph_id } if graph_id.trim().is_empty() => {
            Some(ServerMessage::Error {
                message: "Graph document id must not be empty".to_owned(),
            })
        }
        ClientMessage::SubscribeGraphRuntime { graph_id } => {
            let graph_id = graph_id.trim().to_owned();
            context.runtime_graph_subscriptions.insert(graph_id.clone());
            tracing::trace!(
                client_id = context.client_id,
                graph_id,
                "subscribed to graph runtime stream"
            );
            None
        }
        ClientMessage::UnsubscribeGraphRuntime { graph_id } if graph_id.trim().is_empty() => {
            Some(ServerMessage::Error {
                message: "Graph document id must not be empty".to_owned(),
            })
        }
        ClientMessage::UnsubscribeGraphRuntime { graph_id } => {
            let graph_id = graph_id.trim().to_owned();
            context.runtime_graph_subscriptions.remove(&graph_id);
            tracing::trace!(
                client_id = context.client_id,
                graph_id,
                "unsubscribed from graph runtime stream"
            );
            None
        }
        _ => unreachable!("runtime handler received unsupported message"),
    }
}
