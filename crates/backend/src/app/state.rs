use std::sync::{Arc, atomic::AtomicUsize};

use shared::WledInstance;
use tokio::sync::RwLock;

use crate::messaging::event_bus::EventBus;
use crate::node_runtime::NodeRegistry;
use crate::services::graph_store::GraphStore;
use crate::services::mqtt::HomeAssistantMqttService;
use crate::services::mqtt_broker_store::MqttBrokerStore;
use crate::services::runtime::manager::GraphRuntimeManager;

#[derive(Clone)]
/// Holds the shared backend services and global process state used across request handlers.
pub(crate) struct AppState {
    pub(crate) connected_clients: Arc<AtomicUsize>,
    pub(crate) next_client_id: Arc<AtomicUsize>,
    pub(crate) event_bus: EventBus,
    pub(crate) node_registry: Arc<NodeRegistry>,
    pub(crate) graph_store: Arc<GraphStore>,
    pub(crate) mqtt_broker_store: Arc<MqttBrokerStore>,
    pub(crate) mqtt_service: Arc<HomeAssistantMqttService>,
    pub(crate) runtime_manager: Arc<GraphRuntimeManager>,
    pub(crate) wled_instances: Arc<RwLock<Vec<WledInstance>>>,
}
