use std::{
    env,
    path::PathBuf,
    sync::{Arc, atomic::AtomicUsize},
};

use tokio::sync::RwLock;

use crate::app::state::AppState;
use crate::messaging::event_bus::EventBus;
use crate::node_runtime::build_node_registry;
use crate::services::graph_store::GraphStore;
use crate::services::image_asset_store::{ImageAssetStore, set_global_image_asset_store};
use crate::services::mqtt::{HomeAssistantMqttService, set_global_home_assistant_mqtt_service};
use crate::services::mqtt_broker_store::MqttBrokerStore;
use crate::services::runtime::manager::GraphRuntimeManager;
use crate::services::wled::discovery::spawn_wled_discovery_task;

/// Builds the fully initialized backend application state used by the HTTP and WebSocket layers.
///
/// Startup loads persisted graph and MQTT configuration, constructs the runtime manager and node
/// registry, starts WLED discovery, and restores persisted runtime state before serving requests.
pub(crate) async fn build_app_state() -> anyhow::Result<AppState> {
    let data_dir = app_data_dir();
    let event_bus = EventBus::default();
    let node_registry = build_node_registry()?;
    let graph_store = Arc::new(GraphStore::new(&data_dir, Arc::new(event_bus.clone())));
    let image_asset_store = Arc::new(ImageAssetStore::new(&data_dir)?);
    let mqtt_broker_store = Arc::new(MqttBrokerStore::new(&data_dir));
    let mqtt_service = HomeAssistantMqttService::new();
    mqtt_service.sync_brokers(mqtt_broker_store.list().await?)?;
    set_global_image_asset_store(image_asset_store.clone())?;
    set_global_home_assistant_mqtt_service(mqtt_service.clone())?;
    let runtime_manager = Arc::new(GraphRuntimeManager::new(
        &data_dir,
        node_registry.clone(),
        graph_store.clone(),
        Arc::new(event_bus.clone()),
    ));
    let wled_instances = Arc::new(RwLock::new(Vec::new()));
    spawn_wled_discovery_task(wled_instances.clone(), event_bus.clone());
    runtime_manager.load_persisted_state().await?;

    Ok(AppState {
        connected_clients: Arc::new(AtomicUsize::new(0)),
        next_client_id: Arc::new(AtomicUsize::new(0)),
        event_bus,
        node_registry,
        graph_store,
        image_asset_store,
        mqtt_broker_store,
        mqtt_service,
        runtime_manager,
        wled_instances,
    })
}

/// Returns the runtime data directory used for persisted graphs, broker configs, and runtime state.
///
/// Docker and other deployment environments can override the default location with the
/// `APP_DATA_DIR` environment variable.
fn app_data_dir() -> PathBuf {
    env::var("APP_DATA_DIR")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data"))
}
