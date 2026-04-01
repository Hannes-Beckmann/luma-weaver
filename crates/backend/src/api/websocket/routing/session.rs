use shared::{ClientMessage, EventMessage, EventScope, EventTopic, ServerMessage};

use super::{RoutingContext, sorted_subscriptions};

/// Handles connection-scoped session messages such as ping, display name, and event subscriptions.
///
/// These messages do not touch graph or runtime state; they only emit connection events and update
/// the per-client subscription set maintained by the WebSocket session.
pub(super) async fn handle(
    context: &mut RoutingContext<'_>,
    message: ClientMessage,
) -> Option<ServerMessage> {
    match message {
        ClientMessage::Ping => {
            context.state.event_bus.emit_event(EventMessage {
                topic: EventTopic::Ping,
                scope: EventScope::Global,
                message: format!("Client {} sent ping", context.client_id),
            });
            Some(ServerMessage::Pong)
        }
        ClientMessage::SetName { name } if name.trim().is_empty() => {
            tracing::warn!(client_id = context.client_id, "received empty name update");
            Some(ServerMessage::Error {
                message: "Name must not be empty".to_owned(),
            })
        }
        ClientMessage::SetName { name } => {
            tracing::info!(client_id = context.client_id, name = %name, "received name update");
            context.state.event_bus.emit_event(EventMessage {
                topic: EventTopic::Name,
                scope: EventScope::Global,
                message: format!("Client {} set name to {name}", context.client_id),
            });
            Some(ServerMessage::Welcome {
                message: format!("Hello, {name}"),
            })
        }
        ClientMessage::Subscribe {
            subscriptions: new_subscriptions,
        } => {
            context.subscriptions.extend(new_subscriptions);
            tracing::debug!(
                client_id = context.client_id,
                active_subscriptions = context.subscriptions.len(),
                "updated client event subscriptions"
            );
            Some(ServerMessage::SubscriptionState {
                subscriptions: sorted_subscriptions(context.subscriptions),
            })
        }
        ClientMessage::Unsubscribe {
            subscriptions: removed_subscriptions,
        } => {
            for subscription in removed_subscriptions {
                context.subscriptions.remove(&subscription);
            }
            tracing::debug!(
                client_id = context.client_id,
                active_subscriptions = context.subscriptions.len(),
                "updated client event subscriptions"
            );
            Some(ServerMessage::SubscriptionState {
                subscriptions: sorted_subscriptions(context.subscriptions),
            })
        }
        _ => unreachable!("session handler received unsupported message"),
    }
}
