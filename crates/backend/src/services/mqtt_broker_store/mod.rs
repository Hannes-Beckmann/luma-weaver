use std::path::{Path, PathBuf};

use anyhow::Context;
use shared::MqttBrokerConfig;
use tokio::sync::Mutex;

/// Persists reusable MQTT broker configurations in a single JSON file.
pub(crate) struct MqttBrokerStore {
    path: PathBuf,
    lock: Mutex<()>,
}

impl MqttBrokerStore {
    /// Builds a broker store rooted under the backend data directory.
    pub(crate) fn new(root_dir: &Path) -> Self {
        Self {
            path: root_dir.join("mqtt_brokers.json"),
            lock: Mutex::new(()),
        }
    }

    /// Loads all persisted MQTT broker configurations.
    ///
    /// Missing storage is treated as an empty broker list rather than an error.
    pub(crate) async fn list(&self) -> anyhow::Result<Vec<MqttBrokerConfig>> {
        let _guard = self.lock.lock().await;
        let payload = match tokio::fs::read_to_string(&self.path).await {
            Ok(payload) => payload,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => {
                return Err(error).with_context(|| format!("read {}", self.path.display()));
            }
        };

        serde_json::from_str::<Vec<MqttBrokerConfig>>(&payload)
            .with_context(|| format!("parse {}", self.path.display()))
    }

    /// Replaces the persisted MQTT broker configuration file with the provided broker list.
    pub(crate) async fn save_all(&self, brokers: &[MqttBrokerConfig]) -> anyhow::Result<()> {
        let _guard = self.lock.lock().await;
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("create {}", parent.display()))?;
        }
        let payload = serde_json::to_vec_pretty(brokers).context("serialize mqtt brokers")?;
        tokio::fs::write(&self.path, payload)
            .await
            .with_context(|| format!("write {}", self.path.display()))
    }
}
