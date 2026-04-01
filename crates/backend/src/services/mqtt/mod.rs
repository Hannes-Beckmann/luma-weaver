use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};
use std::time::{Duration, Instant};

use anyhow::Context;
use flume::{Receiver, Sender};
use rumqttc::{AsyncClient, Event, LastWill, MqttOptions, Packet, QoS};
use shared::MqttBrokerConfig;
use tokio::time::MissedTickBehavior;

static GLOBAL_HOME_ASSISTANT_MQTT_SERVICE: OnceLock<Arc<HomeAssistantMqttService>> =
    OnceLock::new();

const DEFAULT_STATE_PUBLISH_DELAY: Duration = Duration::from_millis(750);

/// Registers the global Home Assistant MQTT service instance.
///
/// The service is stored once for process-wide access by runtime nodes.
pub(crate) fn set_global_home_assistant_mqtt_service(
    service: Arc<HomeAssistantMqttService>,
) -> anyhow::Result<()> {
    GLOBAL_HOME_ASSISTANT_MQTT_SERVICE
        .set(service)
        .map_err(|_| anyhow::anyhow!("global Home Assistant MQTT service already initialized"))
}

/// Returns the global Home Assistant MQTT service instance when it has been initialized.
pub(crate) fn global_home_assistant_mqtt_service() -> Option<&'static Arc<HomeAssistantMqttService>>
{
    GLOBAL_HOME_ASSISTANT_MQTT_SERVICE.get()
}

#[derive(Clone, PartialEq)]
pub(crate) struct HaMqttNumberRegistration {
    pub(crate) entity_id: String,
    pub(crate) display_name: String,
    pub(crate) default_value: f32,
    pub(crate) min: f32,
    pub(crate) max: f32,
    pub(crate) step: f32,
    pub(crate) retain: bool,
}

pub(crate) struct HaMqttNumberSnapshot {
    pub(crate) connected: bool,
    pub(crate) value: f32,
    pub(crate) waiting_for_first_value: bool,
}

pub(crate) struct HomeAssistantMqttService {
    brokers: RwLock<HashMap<String, BrokerHandle>>,
}

impl HomeAssistantMqttService {
    /// Creates a new shared Home Assistant MQTT service.
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            brokers: RwLock::new(HashMap::new()),
        })
    }

    /// Reconciles the active broker tasks with the latest configured broker list.
    ///
    /// Removed brokers are stopped, unchanged brokers are kept, and changed broker definitions are
    /// recreated with fresh runtime state.
    pub(crate) fn sync_brokers(&self, configs: Vec<MqttBrokerConfig>) -> anyhow::Result<()> {
        let mut brokers = self
            .brokers
            .write()
            .map_err(|_| anyhow::anyhow!("mqtt broker registry lock poisoned"))?;

        let mut configs_by_id = HashMap::new();
        for config in configs {
            anyhow::ensure!(
                !config.id.trim().is_empty(),
                "MQTT broker id must not be empty"
            );
            anyhow::ensure!(
                !configs_by_id.contains_key(config.id.as_str()),
                "Duplicate MQTT broker id {}",
                config.id
            );
            configs_by_id.insert(config.id.clone(), config);
        }

        let existing_ids = brokers.keys().cloned().collect::<Vec<_>>();
        for broker_id in existing_ids {
            if !configs_by_id.contains_key(broker_id.as_str()) {
                if let Some(handle) = brokers.remove(&broker_id) {
                    let _ = handle.command_tx.send(BrokerCommand::Stop);
                }
            }
        }

        for (broker_id, config) in configs_by_id {
            let recreate = brokers
                .get(&broker_id)
                .is_none_or(|handle| handle.config != config);
            if !recreate {
                continue;
            }

            if let Some(previous) = brokers.remove(&broker_id) {
                let _ = previous.command_tx.send(BrokerCommand::Stop);
            }

            let state = Arc::new(RwLock::new(BrokerRuntimeState::default()));
            let (command_tx, command_rx) = flume::unbounded();
            spawn_broker_task(config.clone(), state.clone(), command_rx);
            brokers.insert(
                broker_id,
                BrokerHandle {
                    config,
                    state,
                    command_tx,
                },
            );
        }

        Ok(())
    }

    /// Ensures that a Home Assistant `number` entity exists for the given broker and returns its
    /// latest known snapshot.
    ///
    /// New registrations trigger discovery publication and, after a short delay, publish the
    /// configured default value when Home Assistant has not supplied a retained state yet.
    pub(crate) fn ensure_number_entity(
        &self,
        broker_id: &str,
        registration: HaMqttNumberRegistration,
    ) -> anyhow::Result<HaMqttNumberSnapshot> {
        let brokers = self
            .brokers
            .read()
            .map_err(|_| anyhow::anyhow!("mqtt broker registry lock poisoned"))?;
        let handle = brokers
            .get(broker_id)
            .ok_or_else(|| anyhow::anyhow!("Unknown MQTT broker {}", broker_id))?;

        let mut should_upsert = false;
        let mut should_publish_default = false;
        let snapshot = {
            let mut state = handle
                .state
                .write()
                .map_err(|_| anyhow::anyhow!("mqtt broker state lock poisoned"))?;
            let connected = state.connected;
            let entity = state
                .numbers
                .entry(registration.entity_id.clone())
                .or_insert_with(|| NumberEntityState {
                    registration: registration.clone(),
                    latest_value: None,
                    first_registered_at: Instant::now(),
                    default_published: false,
                });

            if entity.registration != registration {
                entity.registration = registration.clone();
                entity.first_registered_at = Instant::now();
                entity.default_published = false;
                should_upsert = true;
            } else if entity.latest_value.is_none() {
                should_upsert = true;
            }

            if entity.latest_value.is_none()
                && !entity.default_published
                && entity.first_registered_at.elapsed() >= DEFAULT_STATE_PUBLISH_DELAY
            {
                entity.default_published = true;
                entity.latest_value = Some(registration.default_value);
                should_publish_default = true;
            }

            HaMqttNumberSnapshot {
                connected,
                value: entity.latest_value.unwrap_or(registration.default_value),
                waiting_for_first_value: entity.latest_value.is_none(),
            }
        };

        if should_upsert {
            handle
                .command_tx
                .send(BrokerCommand::UpsertNumberEntity(registration.clone()))
                .context("send MQTT entity registration command")?;
        }
        if should_publish_default {
            handle
                .command_tx
                .send(BrokerCommand::PublishNumberState {
                    entity_id: registration.entity_id,
                    value: snapshot.value,
                    retain: registration.retain,
                })
                .context("send MQTT default state publish command")?;
        }

        Ok(snapshot)
    }
}

#[derive(Clone)]
struct BrokerHandle {
    config: MqttBrokerConfig,
    state: Arc<RwLock<BrokerRuntimeState>>,
    command_tx: Sender<BrokerCommand>,
}

#[derive(Default)]
struct BrokerRuntimeState {
    connected: bool,
    numbers: HashMap<String, NumberEntityState>,
}

struct NumberEntityState {
    registration: HaMqttNumberRegistration,
    latest_value: Option<f32>,
    first_registered_at: Instant,
    default_published: bool,
}

enum BrokerCommand {
    UpsertNumberEntity(HaMqttNumberRegistration),
    PublishNumberState {
        entity_id: String,
        value: f32,
        retain: bool,
    },
    Stop,
}

/// Spawns the asynchronous MQTT broker task for a single configured broker.
///
/// The task owns the MQTT client, processes broker commands, mirrors connection state into
/// `BrokerRuntimeState`, and relays command/state messages for registered entities.
fn spawn_broker_task(
    config: MqttBrokerConfig,
    state: Arc<RwLock<BrokerRuntimeState>>,
    command_rx: Receiver<BrokerCommand>,
) {
    tokio::spawn(async move {
        let availability_topic = availability_topic(&config.id);
        let mut options = MqttOptions::new(
            format!("luma-weaver-{}", sanitize_identifier(&config.id)),
            config.host.clone(),
            config.port,
        );
        options.set_keep_alive(Duration::from_secs(15));
        options.set_last_will(LastWill::new(
            availability_topic.clone(),
            "offline",
            QoS::AtLeastOnce,
            true,
        ));
        if !config.username.trim().is_empty() {
            options.set_credentials(config.username.clone(), config.password.clone());
        }

        let (client, mut eventloop) = AsyncClient::new(options, 32);
        let mut ticker = tokio::time::interval(Duration::from_millis(50));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    while let Ok(command) = command_rx.try_recv() {
                        if handle_broker_command(&client, &config, &state, command).await {
                            let _ = update_connected(&state, false);
                            return;
                        }
                    }
                }
                command = command_rx.recv_async() => {
                    match command {
                        Ok(command) => {
                            if handle_broker_command(&client, &config, &state, command).await {
                                let _ = update_connected(&state, false);
                                return;
                            }
                        }
                        Err(_) => {
                            let _ = update_connected(&state, false);
                            return;
                        }
                    }
                }
                event = eventloop.poll() => {
                    match event {
                        Ok(Event::Incoming(Packet::ConnAck(_))) => {
                            let _ = update_connected(&state, true);
                            let _ = client
                                .publish(&availability_topic, QoS::AtLeastOnce, true, "online")
                                .await;
                        }
                        Ok(Event::Incoming(Packet::Publish(publish))) => {
                            handle_publish(&config, &state, &client, publish.topic, publish.payload.to_vec()).await;
                        }
                        Ok(_) => {}
                        Err(error) => {
                            tracing::warn!(broker_id = %config.id, %error, "MQTT broker event loop error");
                            let _ = update_connected(&state, false);
                            tokio::time::sleep(Duration::from_secs(1)).await;
                        }
                    }
                }
            }
        }
    });
}

/// Processes a command sent to a broker task.
///
/// Returns `true` when the caller should stop the broker task.
async fn handle_broker_command(
    client: &AsyncClient,
    config: &MqttBrokerConfig,
    state: &Arc<RwLock<BrokerRuntimeState>>,
    command: BrokerCommand,
) -> bool {
    match command {
        BrokerCommand::UpsertNumberEntity(registration) => {
            let command_topic = number_command_topic(&config.id, &registration.entity_id);
            let state_topic = number_state_topic(&config.id, &registration.entity_id);
            let cached_value = state.read().ok().and_then(|current_state| {
                current_state
                    .numbers
                    .get(&registration.entity_id)
                    .and_then(|entity_state| entity_state.latest_value)
            });
            let payload = serde_json::json!({
                "name": registration.display_name,
                "unique_id": format!(
                    "animation_builder_{}_{}",
                    sanitize_identifier(&config.id),
                    sanitize_identifier(&registration.entity_id),
                ),
                "object_id": sanitize_identifier(&registration.entity_id),
                "state_topic": state_topic,
                "command_topic": command_topic,
                "availability_topic": availability_topic(&config.id),
                "min": registration.min,
                "max": registration.max,
                "step": registration.step,
                "mode": "box",
                "device": {
                    "identifiers": [format!("animation_builder_{}", sanitize_identifier(&config.id))],
                    "name": "Luma Weaver",
                    "manufacturer": "Luma Weaver",
                    "model": "Graph Controls",
                }
            });
            if let Err(error) = client
                .subscribe(command_topic.clone(), QoS::AtLeastOnce)
                .await
            {
                tracing::warn!(broker_id = %config.id, %error, topic = %command_topic, "failed to subscribe to Home Assistant command topic");
            }
            if let Err(error) = client
                .subscribe(state_topic.clone(), QoS::AtLeastOnce)
                .await
            {
                tracing::warn!(broker_id = %config.id, %error, topic = %state_topic, "failed to subscribe to Home Assistant state topic");
            }
            if let Err(error) = client
                .publish(
                    discovery_topic(config, &registration.entity_id),
                    QoS::AtLeastOnce,
                    true,
                    payload.to_string(),
                )
                .await
            {
                tracing::warn!(broker_id = %config.id, %error, entity_id = %registration.entity_id, "failed to publish Home Assistant discovery payload");
            }

            if let Some(value) = cached_value {
                let _ = client
                    .publish(
                        state_topic,
                        QoS::AtLeastOnce,
                        registration.retain,
                        value.to_string(),
                    )
                    .await;
            }
            false
        }
        BrokerCommand::PublishNumberState {
            entity_id,
            value,
            retain,
        } => {
            if let Err(error) = client
                .publish(
                    number_state_topic(&config.id, &entity_id),
                    QoS::AtLeastOnce,
                    retain,
                    value.to_string(),
                )
                .await
            {
                tracing::warn!(broker_id = %config.id, %error, %entity_id, "failed to publish Home Assistant number state");
            }
            false
        }
        BrokerCommand::Stop => true,
    }
}

/// Processes an incoming MQTT publish for a registered entity.
///
/// Command-topic updates are mirrored back to the state topic so Home Assistant sees the confirmed
/// value as the entity's current state.
async fn handle_publish(
    config: &MqttBrokerConfig,
    state: &Arc<RwLock<BrokerRuntimeState>>,
    client: &AsyncClient,
    topic: String,
    payload: Vec<u8>,
) {
    let value = match std::str::from_utf8(&payload)
        .ok()
        .map(str::trim)
        .and_then(|payload| payload.parse::<f32>().ok())
    {
        Some(value) => value,
        None => {
            tracing::warn!(broker_id = %config.id, topic = %topic, "failed to parse MQTT number payload");
            return;
        }
    };

    let maybe_entity = {
        let mut state = match state.write() {
            Ok(state) => state,
            Err(_) => return,
        };
        let entity_id = state.numbers.keys().find_map(|entity_id| {
            if topic == number_command_topic(&config.id, entity_id)
                || topic == number_state_topic(&config.id, entity_id)
            {
                Some(entity_id.clone())
            } else {
                None
            }
        });
        if let Some(entity_id) = entity_id.clone() {
            if let Some(entity_state) = state.numbers.get_mut(&entity_id) {
                entity_state.latest_value = Some(value);
            }
        }
        entity_id
    };

    if let Some(entity_id) = maybe_entity {
        if topic == number_command_topic(&config.id, &entity_id) {
            let retain = state
                .read()
                .ok()
                .and_then(|state| {
                    state
                        .numbers
                        .get(&entity_id)
                        .map(|entry| entry.registration.retain)
                })
                .unwrap_or(true);
            let _ = client
                .publish(
                    number_state_topic(&config.id, &entity_id),
                    QoS::AtLeastOnce,
                    retain,
                    value.to_string(),
                )
                .await;
        }
    }
}

/// Updates the cached connection state for a broker runtime.
fn update_connected(
    state: &Arc<RwLock<BrokerRuntimeState>>,
    connected: bool,
) -> anyhow::Result<()> {
    let mut state = state
        .write()
        .map_err(|_| anyhow::anyhow!("mqtt broker state lock poisoned"))?;
    state.connected = connected;
    Ok(())
}

/// Returns the Home Assistant discovery topic for a number entity.
fn discovery_topic(config: &MqttBrokerConfig, entity_id: &str) -> String {
    format!(
        "{}/number/animation_builder/{}/config",
        config.discovery_prefix.trim().trim_end_matches('/'),
        sanitize_identifier(entity_id)
    )
}

/// Returns the MQTT state topic for a Home Assistant number entity.
fn number_state_topic(broker_id: &str, entity_id: &str) -> String {
    format!(
        "animation_builder/{}/number/{}/state",
        sanitize_identifier(broker_id),
        sanitize_identifier(entity_id)
    )
}

/// Returns the MQTT command topic for a Home Assistant number entity.
fn number_command_topic(broker_id: &str, entity_id: &str) -> String {
    format!(
        "animation_builder/{}/number/{}/set",
        sanitize_identifier(broker_id),
        sanitize_identifier(entity_id)
    )
}

/// Returns the MQTT availability topic for a broker-backed entity set.
fn availability_topic(broker_id: &str) -> String {
    format!(
        "animation_builder/{}/availability",
        sanitize_identifier(broker_id)
    )
}

/// Sanitizes an identifier for use in MQTT topics and Home Assistant object IDs.
///
/// Non-alphanumeric characters are replaced with underscores and the result is lowercased.
fn sanitize_identifier(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            output.push(ch.to_ascii_lowercase());
        } else {
            output.push('_');
        }
    }
    if output.is_empty() {
        "animation_builder".to_owned()
    } else {
        output
    }
}
