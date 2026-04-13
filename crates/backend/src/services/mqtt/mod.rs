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
    pub(crate) graph_id: String,
    pub(crate) graph_name: String,
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

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct NumberEntityKey {
    graph_id: String,
    entity_id: String,
}

impl NumberEntityKey {
    fn from_registration(registration: &HaMqttNumberRegistration) -> Self {
        Self {
            graph_id: registration.graph_id.clone(),
            entity_id: registration.entity_id.clone(),
        }
    }
}

pub(crate) struct HomeAssistantMqttService {
    configs: RwLock<HashMap<String, MqttBrokerConfig>>,
    brokers: RwLock<HashMap<String, BrokerHandle>>,
}

impl HomeAssistantMqttService {
    /// Creates a new shared Home Assistant MQTT service.
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            configs: RwLock::new(HashMap::new()),
            brokers: RwLock::new(HashMap::new()),
        })
    }

    /// Reconciles the active broker tasks with the latest configured broker list.
    ///
    /// Removed brokers are stopped, unchanged brokers are kept, and changed broker definitions are
    /// recreated with fresh runtime state. Only brokers marked as Home Assistant brokers are
    /// activated for the runtime service.
    pub(crate) fn sync_brokers(&self, configs: Vec<MqttBrokerConfig>) -> anyhow::Result<()> {
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

        let active_configs = configs_by_id
            .iter()
            .filter(|(_, config)| config.is_home_assistant)
            .map(|(broker_id, config)| (broker_id.clone(), config.clone()))
            .collect::<HashMap<_, _>>();

        {
            let mut stored_configs = self
                .configs
                .write()
                .map_err(|_| anyhow::anyhow!("mqtt broker config registry lock poisoned"))?;
            *stored_configs = configs_by_id;
        }

        let mut brokers = self
            .brokers
            .write()
            .map_err(|_| anyhow::anyhow!("mqtt broker registry lock poisoned"))?;

        let existing_ids = brokers.keys().cloned().collect::<Vec<_>>();
        for broker_id in existing_ids {
            if !active_configs.contains_key(broker_id.as_str()) {
                if let Some(handle) = brokers.remove(&broker_id) {
                    let _ = handle.command_tx.send(BrokerCommand::Stop);
                }
            }
        }

        for (broker_id, config) in active_configs {
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
        {
            let configs = self
                .configs
                .read()
                .map_err(|_| anyhow::anyhow!("mqtt broker config registry lock poisoned"))?;
            let config = configs
                .get(broker_id)
                .ok_or_else(|| anyhow::anyhow!("Unknown MQTT broker {}", broker_id))?;
            anyhow::ensure!(
                config.is_home_assistant,
                "MQTT broker {} is not marked as a Home Assistant broker",
                broker_id
            );
        }

        let brokers = self
            .brokers
            .read()
            .map_err(|_| anyhow::anyhow!("mqtt broker registry lock poisoned"))?;
        let handle = brokers.get(broker_id).ok_or_else(|| {
            anyhow::anyhow!("Home Assistant MQTT broker {} is not active", broker_id)
        })?;

        let mut should_upsert = false;
        let mut should_publish_default = false;
        let snapshot = {
            let mut state = handle
                .state
                .write()
                .map_err(|_| anyhow::anyhow!("mqtt broker state lock poisoned"))?;
            let connected = state.connected;
            let key = NumberEntityKey::from_registration(&registration);
            let entity = state
                .numbers
                .entry(key)
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
                    graph_id: registration.graph_id,
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

#[cfg(test)]
mod tests {
    use super::{
        BrokerRuntimeState, HaMqttNumberRegistration, HomeAssistantMqttService, NumberEntityKey,
        NumberEntityState, device_identifier, discovery_topic, entity_object_id, entity_unique_id,
        graph_display_name, handle_publish, number_command_topic, number_state_topic,
    };
    use rumqttc::{AsyncClient, MqttOptions};
    use shared::MqttBrokerConfig;
    use std::collections::HashMap;
    use std::sync::{Arc, RwLock};
    use std::time::Instant;

    fn broker_config(id: &str, is_home_assistant: bool) -> MqttBrokerConfig {
        MqttBrokerConfig {
            id: id.to_owned(),
            display_name: id.to_owned(),
            host: "127.0.0.1".to_owned(),
            port: 1883,
            username: String::new(),
            password: String::new(),
            discovery_prefix: "homeassistant".to_owned(),
            is_home_assistant,
        }
    }

    fn registration() -> HaMqttNumberRegistration {
        HaMqttNumberRegistration {
            graph_id: "graph_one".to_owned(),
            graph_name: "Graph One".to_owned(),
            entity_id: "number_one".to_owned(),
            display_name: "Number One".to_owned(),
            default_value: 42.0,
            min: 0.0,
            max: 100.0,
            step: 1.0,
            retain: true,
        }
    }

    #[tokio::test]
    async fn sync_brokers_keeps_generic_brokers_in_storage_but_not_active() {
        let service = HomeAssistantMqttService::new();

        service
            .sync_brokers(vec![broker_config("generic", false)])
            .expect("sync brokers");

        let configs = service.configs.read().expect("read configs");
        assert!(configs.contains_key("generic"));
        drop(configs);

        let brokers = service.brokers.read().expect("read brokers");
        assert!(!brokers.contains_key("generic"));
    }

    #[tokio::test]
    async fn ensure_number_entity_rejects_generic_brokers() {
        let service = HomeAssistantMqttService::new();

        service
            .sync_brokers(vec![broker_config("generic", false)])
            .expect("sync brokers");

        let error = match service.ensure_number_entity("generic", registration()) {
            Ok(_) => panic!("generic broker should be rejected"),
            Err(error) => error,
        };
        assert!(
            error
                .to_string()
                .contains("not marked as a Home Assistant broker")
        );
    }

    #[test]
    fn graph_scoped_identifiers_use_graph_identity() {
        let config = broker_config("primary", true);
        let registration = registration();

        assert_eq!(
            discovery_topic(&config, &registration.graph_id, &registration.entity_id),
            "homeassistant/number/luma_weaver/graph_one_number_one/config"
        );
        assert_eq!(
            number_state_topic(&config.id, &registration.graph_id, &registration.entity_id),
            "luma_weaver/primary/graph/graph_one/number/number_one/state"
        );
        assert_eq!(
            number_command_topic(&config.id, &registration.graph_id, &registration.entity_id),
            "luma_weaver/primary/graph/graph_one/number/number_one/set"
        );
        assert_eq!(
            device_identifier(&registration.graph_id),
            "luma_weaver_graph_graph_one"
        );
        assert_eq!(
            entity_object_id(&registration.graph_id, &registration.entity_id),
            "graph_one_number_one"
        );
        assert_eq!(
            entity_unique_id(&config.id, &registration.graph_id, &registration.entity_id),
            "luma_weaver_primary_graph_one_number_one"
        );
    }

    #[test]
    fn graph_display_name_falls_back_when_graph_name_is_empty() {
        let mut registration = registration();
        assert_eq!(graph_display_name(&registration), "Graph One");

        registration.graph_name.clear();
        assert_eq!(graph_display_name(&registration), "Luma Weaver Graph");
    }

    #[tokio::test]
    async fn ensure_number_entity_keeps_same_entity_id_separate_per_graph() {
        let service = HomeAssistantMqttService::new();
        service
            .sync_brokers(vec![broker_config("primary", true)])
            .expect("sync brokers");

        let registration_one = registration();
        let mut registration_two = registration();
        registration_two.graph_id = "graph_two".to_owned();
        registration_two.graph_name = "Graph Two".to_owned();

        service
            .ensure_number_entity("primary", registration_one.clone())
            .expect("register first entity");
        service
            .ensure_number_entity("primary", registration_two.clone())
            .expect("register second entity");

        let brokers = service.brokers.read().expect("read brokers");
        let handle = brokers.get("primary").expect("active broker");
        let state = handle.state.read().expect("read broker state");

        assert!(
            state
                .numbers
                .contains_key(&NumberEntityKey::from_registration(&registration_one))
        );
        assert!(
            state
                .numbers
                .contains_key(&NumberEntityKey::from_registration(&registration_two))
        );
        assert_eq!(state.numbers.len(), 2);
    }

    #[tokio::test]
    async fn handle_publish_updates_only_matching_graph_entity() {
        let config = broker_config("primary", true);
        let first = registration();
        let mut second = registration();
        second.graph_id = "graph_two".to_owned();
        second.graph_name = "Graph Two".to_owned();

        let state = Arc::new(RwLock::new(BrokerRuntimeState {
            connected: true,
            numbers: HashMap::from([
                (
                    NumberEntityKey::from_registration(&first),
                    NumberEntityState {
                        registration: first.clone(),
                        latest_value: Some(1.0),
                        first_registered_at: Instant::now(),
                        default_published: true,
                    },
                ),
                (
                    NumberEntityKey::from_registration(&second),
                    NumberEntityState {
                        registration: second.clone(),
                        latest_value: Some(2.0),
                        first_registered_at: Instant::now(),
                        default_published: true,
                    },
                ),
            ]),
        }));
        let (client, _eventloop) =
            AsyncClient::new(MqttOptions::new("mqtt-test-client", "127.0.0.1", 1883), 10);

        handle_publish(
            &config,
            &state,
            &client,
            number_state_topic(&config.id, &second.graph_id, &second.entity_id),
            b"7.5".to_vec(),
        )
        .await;

        let state = state.read().expect("read broker state");
        assert_eq!(
            state
                .numbers
                .get(&NumberEntityKey::from_registration(&first))
                .and_then(|entry| entry.latest_value),
            Some(1.0)
        );
        assert_eq!(
            state
                .numbers
                .get(&NumberEntityKey::from_registration(&second))
                .and_then(|entry| entry.latest_value),
            Some(7.5)
        );
    }
}

#[derive(Default)]
struct BrokerRuntimeState {
    connected: bool,
    numbers: HashMap<NumberEntityKey, NumberEntityState>,
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
        graph_id: String,
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
        let availability_topic = broker_availability_topic(&config.id);
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
            let state_topic =
                number_state_topic(&config.id, &registration.graph_id, &registration.entity_id);
            let command_topic =
                number_command_topic(&config.id, &registration.graph_id, &registration.entity_id);
            let key = NumberEntityKey::from_registration(&registration);
            let cached_value = state.read().ok().and_then(|current_state| {
                current_state
                    .numbers
                    .get(&key)
                    .and_then(|entity_state| entity_state.latest_value)
            });
            let payload = serde_json::json!({
                "name": registration.display_name,
                "unique_id": entity_unique_id(&config.id, &registration.graph_id, &registration.entity_id),
                "object_id": entity_object_id(&registration.graph_id, &registration.entity_id),
                "state_topic": state_topic,
                "command_topic": command_topic,
                "availability_topic": broker_availability_topic(&config.id),
                "min": registration.min,
                "max": registration.max,
                "step": registration.step,
                "mode": "box",
                "device": {
                    "identifiers": [device_identifier(&registration.graph_id)],
                    "name": graph_display_name(&registration),
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
                    discovery_topic(config, &registration.graph_id, &registration.entity_id),
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
            graph_id,
            entity_id,
            value,
            retain,
        } => {
            if let Err(error) = client
                .publish(
                    number_state_topic(&config.id, &graph_id, &entity_id),
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
        let entity_key = state.numbers.iter().find_map(|(entity_key, entity_state)| {
            if topic
                == number_command_topic(
                    &config.id,
                    &entity_state.registration.graph_id,
                    &entity_key.entity_id,
                )
                || topic
                    == number_state_topic(
                        &config.id,
                        &entity_state.registration.graph_id,
                        &entity_key.entity_id,
                    )
            {
                Some(entity_key.clone())
            } else {
                None
            }
        });
        if let Some(entity_key) = entity_key.clone() {
            if let Some(entity_state) = state.numbers.get_mut(&entity_key) {
                entity_state.latest_value = Some(value);
            }
        }
        entity_key
    };

    if let Some(entity_key) = maybe_entity {
        if topic == number_command_topic(&config.id, &entity_key.graph_id, &entity_key.entity_id) {
            let retain = state.read().ok().and_then(|state| {
                state
                    .numbers
                    .get(&entity_key)
                    .map(|entry| entry.registration.retain)
            });
            let _ = client
                .publish(
                    number_state_topic(&config.id, &entity_key.graph_id, &entity_key.entity_id),
                    QoS::AtLeastOnce,
                    retain.unwrap_or(true),
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
fn discovery_topic(config: &MqttBrokerConfig, graph_id: &str, entity_id: &str) -> String {
    format!(
        "{}/number/luma_weaver/{}/config",
        config.discovery_prefix.trim().trim_end_matches('/'),
        entity_object_id(graph_id, entity_id)
    )
}

/// Returns the MQTT state topic for a Home Assistant number entity.
fn number_state_topic(broker_id: &str, graph_id: &str, entity_id: &str) -> String {
    format!(
        "luma_weaver/{}/graph/{}/number/{}/state",
        sanitize_identifier(broker_id),
        sanitize_identifier(graph_id),
        sanitize_identifier(entity_id)
    )
}

/// Returns the MQTT command topic for a Home Assistant number entity.
fn number_command_topic(broker_id: &str, graph_id: &str, entity_id: &str) -> String {
    format!(
        "luma_weaver/{}/graph/{}/number/{}/set",
        sanitize_identifier(broker_id),
        sanitize_identifier(graph_id),
        sanitize_identifier(entity_id)
    )
}

/// Returns the MQTT availability topic for a broker connection.
fn broker_availability_topic(broker_id: &str) -> String {
    format!(
        "luma_weaver/{}/availability",
        sanitize_identifier(broker_id)
    )
}

/// Returns the stable Home Assistant device identifier for a graph-backed entity set.
fn device_identifier(graph_id: &str) -> String {
    format!("luma_weaver_graph_{}", sanitize_identifier(graph_id))
}

/// Returns the stable Home Assistant object ID for one number entity within a graph.
fn entity_object_id(graph_id: &str, entity_id: &str) -> String {
    format!(
        "{}_{}",
        sanitize_identifier(graph_id),
        sanitize_identifier(entity_id)
    )
}

/// Returns the stable Home Assistant unique ID for one graph-backed entity.
fn entity_unique_id(broker_id: &str, graph_id: &str, entity_id: &str) -> String {
    format!(
        "luma_weaver_{}_{}_{}",
        sanitize_identifier(broker_id),
        sanitize_identifier(graph_id),
        sanitize_identifier(entity_id),
    )
}

/// Returns the display name Home Assistant should show for the graph-backed device.
fn graph_display_name(registration: &HaMqttNumberRegistration) -> String {
    let graph_name = registration.graph_name.trim();
    if graph_name.is_empty() {
        "Luma Weaver Graph".to_owned()
    } else {
        graph_name.to_owned()
    }
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
        "luma_weaver".to_owned()
    } else {
        output
    }
}
