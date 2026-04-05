use std::collections::HashMap;

use anyhow::Result;
use serde_json::Value as JsonValue;
use shared::{NodeDiagnostic, NodeDiagnosticSeverity};

use crate::node_runtime::{
    NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters, TypedNodeEvaluation,
};
use crate::services::mqtt::{HaMqttNumberRegistration, global_home_assistant_mqtt_service};

#[derive(Default)]
pub(crate) struct HomeAssistantMqttNumberNode {
    config: HomeAssistantMqttNumberConfig,
    latest_value: f32,
}

#[derive(Clone)]
struct HomeAssistantMqttNumberConfig {
    broker_id: String,
    entity_id: String,
    display_name: String,
    default_value: f32,
    min: f32,
    max: f32,
    step: f32,
    retain: bool,
}

impl Default for HomeAssistantMqttNumberConfig {
    fn default() -> Self {
        Self {
            broker_id: String::new(),
            entity_id: "animation_builder_number".to_owned(),
            display_name: "Luma Weaver Number".to_owned(),
            default_value: 0.0,
            min: 0.0,
            max: 100.0,
            step: 1.0,
            retain: true,
        }
    }
}

crate::node_runtime::impl_runtime_parameters!(HomeAssistantMqttNumberConfig {
    broker_id: String => |value| value.trim().to_owned(), default String::new(),
    entity_id: String => |value| value.trim().to_owned(), default "animation_builder_number".to_owned(),
    display_name: String => |value| value.trim().to_owned(), default "Luma Weaver Number".to_owned(),
    default_value: f64 => |value| value as f32, default 0.0f32,
    min: f64 => |value| value as f32, default 0.0f32,
    max: f64 => |value| value as f32, default 100.0f32,
    step: f64 => |value| crate::node_runtime::max_f64_to_f32(value, 0.0001), default 1.0f32,
    retain: bool = true,
});

impl HomeAssistantMqttNumberNode {
    fn from_config(config: HomeAssistantMqttNumberConfig) -> Self {
        Self {
            latest_value: config.default_value,
            config,
        }
    }
}

impl RuntimeNodeFromParameters for HomeAssistantMqttNumberNode {
    fn from_parameters(
        parameters: &HashMap<String, JsonValue>,
    ) -> crate::node_runtime::NodeConstruction<Self> {
        let crate::node_runtime::NodeConstruction {
            node: config,
            diagnostics,
        } = HomeAssistantMqttNumberConfig::from_parameters(parameters);
        crate::node_runtime::NodeConstruction {
            node: HomeAssistantMqttNumberNode::from_config(config),
            diagnostics,
        }
    }
}

pub(crate) struct HomeAssistantMqttNumberOutputs {
    value: f32,
}

crate::node_runtime::impl_runtime_outputs!(HomeAssistantMqttNumberOutputs { value });

impl RuntimeNode for HomeAssistantMqttNumberNode {
    type Inputs = ();
    type Outputs = HomeAssistantMqttNumberOutputs;

    fn evaluate(
        &mut self,
        context: &NodeEvaluationContext,
        _inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let mut diagnostics = Vec::new();

        if self.config.broker_id.trim().is_empty() {
            diagnostics.push(NodeDiagnostic {
                severity: NodeDiagnosticSeverity::Warning,
                code: Some("ha_mqtt_number_missing_broker".to_owned()),
                message: "No MQTT broker selected.".to_owned(),
            });
            return Ok(TypedNodeEvaluation {
                outputs: HomeAssistantMqttNumberOutputs {
                    value: self.latest_value,
                },
                frontend_updates: Vec::new(),
                diagnostics,
            });
        }

        if self.config.entity_id.trim().is_empty() {
            diagnostics.push(NodeDiagnostic {
                severity: NodeDiagnosticSeverity::Warning,
                code: Some("ha_mqtt_number_missing_entity_id".to_owned()),
                message: "Entity ID must not be empty.".to_owned(),
            });
            return Ok(TypedNodeEvaluation {
                outputs: HomeAssistantMqttNumberOutputs {
                    value: self.latest_value,
                },
                frontend_updates: Vec::new(),
                diagnostics,
            });
        }

        let Some(service) = global_home_assistant_mqtt_service() else {
            diagnostics.push(NodeDiagnostic {
                severity: NodeDiagnosticSeverity::Error,
                code: Some("ha_mqtt_service_unavailable".to_owned()),
                message: "Home Assistant MQTT service is unavailable.".to_owned(),
            });
            return Ok(TypedNodeEvaluation {
                outputs: HomeAssistantMqttNumberOutputs {
                    value: self.latest_value,
                },
                frontend_updates: Vec::new(),
                diagnostics,
            });
        };

        match service.ensure_number_entity(
            &self.config.broker_id,
            HaMqttNumberRegistration {
                graph_id: context.graph_id.clone(),
                graph_name: context.graph_name.clone(),
                entity_id: self.config.entity_id.clone(),
                display_name: self.config.display_name.clone(),
                default_value: self.config.default_value,
                min: self.config.min,
                max: self.config.max,
                step: self.config.step,
                retain: self.config.retain,
            },
        ) {
            Ok(snapshot) => {
                self.latest_value = snapshot.value;
                if !snapshot.connected {
                    diagnostics.push(NodeDiagnostic {
                        severity: NodeDiagnosticSeverity::Warning,
                        code: Some("ha_mqtt_broker_disconnected".to_owned()),
                        message: format!("MQTT broker {} is disconnected.", self.config.broker_id),
                    });
                } else if snapshot.waiting_for_first_value {
                    diagnostics.push(NodeDiagnostic {
                        severity: NodeDiagnosticSeverity::Info,
                        code: Some("ha_mqtt_number_waiting_for_value".to_owned()),
                        message: format!(
                            "Waiting for Home Assistant number {} to send its first value.",
                            self.config.entity_id
                        ),
                    });
                }
            }
            Err(error) => {
                diagnostics.push(NodeDiagnostic {
                    severity: NodeDiagnosticSeverity::Error,
                    code: Some("ha_mqtt_number_setup_failed".to_owned()),
                    message: error.to_string(),
                });
            }
        }

        Ok(TypedNodeEvaluation {
            outputs: HomeAssistantMqttNumberOutputs {
                value: self.latest_value,
            },
            frontend_updates: Vec::new(),
            diagnostics,
        })
    }
}
