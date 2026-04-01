use std::collections::HashSet;

use shared::{ClientMessage, ServerMessage};

use super::RoutingContext;

/// Handles WebSocket messages for external integration configuration and discovery state.
///
/// This currently covers WLED instance snapshots and MQTT broker configuration reads and writes.
pub(super) async fn handle(
    context: &mut RoutingContext<'_>,
    message: ClientMessage,
) -> Option<ServerMessage> {
    match message {
        ClientMessage::GetWledInstances => {
            let instances = context.state.wled_instances.read().await.clone();
            Some(ServerMessage::WledInstances { instances })
        }
        ClientMessage::GetMqttBrokerConfigs => match context.state.mqtt_broker_store.list().await {
            Ok(brokers) => Some(ServerMessage::MqttBrokerConfigs { brokers }),
            Err(error) => Some(ServerMessage::Error {
                message: format!("Failed to load MQTT broker configs: {error}"),
            }),
        },
        ClientMessage::UpdateMqttBrokerConfigs { brokers } => {
            let mut ids = HashSet::new();
            let has_invalid = brokers.iter().any(|broker| {
                broker.id.trim().is_empty()
                    || broker.host.trim().is_empty()
                    || !ids.insert(broker.id.trim().to_owned())
            });
            if has_invalid {
                Some(ServerMessage::Error {
                    message: "MQTT brokers must have unique non-empty ids and hosts".to_owned(),
                })
            } else {
                match context.state.mqtt_broker_store.save_all(&brokers).await {
                    Ok(()) => match context.state.mqtt_service.sync_brokers(brokers.clone()) {
                        Ok(()) => Some(ServerMessage::MqttBrokerConfigs { brokers }),
                        Err(error) => Some(ServerMessage::Error {
                            message: format!("Failed to apply MQTT broker configs: {error}"),
                        }),
                    },
                    Err(error) => Some(ServerMessage::Error {
                        message: format!("Failed to save MQTT broker configs: {error}"),
                    }),
                }
            }
        }
        _ => unreachable!("integrations handler received unsupported message"),
    }
}
