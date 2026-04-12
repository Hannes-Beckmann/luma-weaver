use std::collections::HashMap;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};

use anyhow::Result;
use serde_json::Value as JsonValue;
use shared::{ColorFrame, InputValue, LedLayout, NodeDiagnostic, NodeDiagnosticSeverity};

use crate::node_runtime::{
    AnyInputValue, NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters,
    TypedNodeEvaluation,
};
use crate::services::wled::ddp;

#[derive(Default)]
pub(crate) struct WledTargetNode {
    led_count: usize,
    target: String,
    transport: Option<WledDdpTransport>,
}

#[derive(Default)]
struct WledTargetParameters {
    led_count: usize,
    target: String,
}

crate::node_runtime::impl_runtime_parameters!(WledTargetParameters {
    led_count: u64 => |value| crate::node_runtime::max_u64_to_usize(value, 1), default 60usize,
    target: String = String::new(),
});

impl WledTargetNode {
    /// Creates a WLED target node from parsed parameters and eagerly resolves the destination.
    fn from_config(config: WledTargetParameters) -> Self {
        let transport = WledDdpTransport::new(&config.target).ok();
        Self {
            led_count: config.led_count,
            target: config.target,
            transport,
        }
    }
}

impl RuntimeNodeFromParameters for WledTargetNode {
    fn from_parameters(
        parameters: &HashMap<String, JsonValue>,
    ) -> crate::node_runtime::NodeConstruction<Self> {
        let crate::node_runtime::NodeConstruction {
            node: config,
            diagnostics,
        } = WledTargetParameters::from_parameters(parameters);
        crate::node_runtime::NodeConstruction {
            node: WledTargetNode::from_config(config),
            diagnostics,
        }
    }
}

pub(crate) struct WledTargetInputs {
    value: Option<AnyInputValue>,
    disable: f32,
}

crate::node_runtime::impl_runtime_inputs!(WledTargetInputs {
    value = None,
    disable = 0.0,
});

impl RuntimeNode for WledTargetNode {
    type Inputs = WledTargetInputs;
    type Outputs = ();

    /// Converts the incoming value into a frame and transmits it to the configured WLED target.
    fn evaluate(
        &mut self,
        context: &NodeEvaluationContext,
        inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let mut diagnostics = Vec::new();
        let disabled = is_disabled(inputs.disable);
        let layout = context.render_layout.clone().unwrap_or(LedLayout {
            id: "wled_target".to_owned(),
            pixel_count: self.led_count,
            width: None,
            height: None,
        });

        let frame_for_transport = match inputs.value.map(|value| value.0) {
            Some(InputValue::ColorFrame(frame)) => frame,
            Some(InputValue::Color(color)) => ColorFrame {
                pixels: vec![color; layout.pixel_count],
                layout,
            },
            _ => ColorFrame {
                layout,
                pixels: Vec::new(),
            },
        };

        if self.target.trim().is_empty() {
            diagnostics.push(NodeDiagnostic {
                severity: NodeDiagnosticSeverity::Warning,
                code: Some("wled_target_missing_target".to_owned()),
                message: "No WLED target configured.".to_owned(),
            });
        }

        diagnostics.extend(self.refresh_transport_if_needed());

        if !disabled {
            if let Some(transport) = &mut self.transport {
                if let Err(error) = transport.send(&frame_for_transport, self.led_count) {
                    tracing::warn!(
                        target = %self.target,
                        %error,
                        "failed to send DDP frame to WLED target"
                    );
                    diagnostics.push(NodeDiagnostic {
                        severity: NodeDiagnosticSeverity::Error,
                        code: Some("wled_target_send_failed".to_owned()),
                        message: format!("Failed to send DDP frame to target {}.", self.target),
                    });
                }
            }
        }

        Ok(TypedNodeEvaluation {
            outputs: (),
            frontend_updates: Vec::new(),
            diagnostics,
        })
    }
}

impl WledTargetNode {
    /// Recreates the DDP transport when the configured target changes or is not yet available.
    fn refresh_transport_if_needed(&mut self) -> Vec<NodeDiagnostic> {
        let mut diagnostics = Vec::new();
        let target = self.target.trim();
        if target.is_empty() {
            self.transport = None;
            return diagnostics;
        }

        let needs_refresh = self
            .transport
            .as_ref()
            .is_none_or(|transport| transport.target_source != target);
        if needs_refresh {
            match WledDdpTransport::new(target) {
                Ok(transport) => self.transport = Some(transport),
                Err(error) => {
                    tracing::warn!(target = %self.target, %error, "failed to configure WLED DDP target");
                    self.transport = None;
                    diagnostics.push(NodeDiagnostic {
                        severity: NodeDiagnosticSeverity::Error,
                        code: Some("wled_target_config_failed".to_owned()),
                        message: format!("Failed to configure WLED target {}.", self.target),
                    });
                }
            }
        }
        diagnostics
    }
}

fn is_disabled(value: f32) -> bool {
    value >= 0.5
}

struct WledDdpTransport {
    target_source: String,
    socket: UdpSocket,
    target: SocketAddr,
    sequence: u8,
}

impl WledDdpTransport {
    /// Resolves the target address and binds the UDP socket used for outbound DDP packets.
    fn new(target: &str) -> Result<Self> {
        let target_source = target.trim().to_owned();
        let target = resolve_target(&target_source)?;
        let socket = ddp::bind_socket()?;
        Ok(Self {
            target_source,
            socket,
            target,
            sequence: 0,
        })
    }

    /// Sends one frame to the configured WLED target and advances the DDP sequence number.
    fn send(&mut self, frame: &ColorFrame, led_count: usize) -> Result<()> {
        let sequence = self.sequence;
        self.sequence = self.sequence.wrapping_add(1);
        ddp::send_frame(&self.socket, self.target, sequence, frame, led_count)
    }
}

/// Resolves a user-supplied host string into a socket address, defaulting to the DDP port.
fn resolve_target(target: &str) -> Result<SocketAddr> {
    let normalized = if target.contains(':') {
        target.to_owned()
    } else {
        format!("{target}:{}", ddp::DDP_PORT)
    };
    normalized
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| anyhow::anyhow!("no socket address resolved for {normalized}"))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use shared::{InputValue, NodeDiagnosticSeverity};

    use super::{WledTargetInputs, WledTargetNode, is_disabled, resolve_target};
    use crate::node_runtime::{NodeEvaluationContext, RuntimeInputs, RuntimeNode};

    fn evaluation_context() -> NodeEvaluationContext {
        NodeEvaluationContext {
            graph_id: "graph".to_owned(),
            graph_name: "Graph".to_owned(),
            elapsed_seconds: 0.0,
            render_layout: None,
        }
    }

    /// Tests that bare hostnames inherit the default DDP port during target resolution.
    #[test]
    fn resolve_target_adds_default_ddp_port() {
        let addr = resolve_target("127.0.0.1").expect("resolve target");
        assert_eq!(addr.port(), 4048);
    }

    #[test]
    fn disable_threshold_matches_issue_contract() {
        assert!(!is_disabled(0.49));
        assert!(is_disabled(0.5));
        assert!(is_disabled(1.0));
    }

    #[test]
    fn disabled_target_still_reports_missing_target_diagnostic() {
        let mut node = WledTargetNode::default();
        let mut runtime_inputs = HashMap::new();
        runtime_inputs.insert("disable".to_owned(), InputValue::Float(1.0));
        let inputs =
            WledTargetInputs::from_runtime_inputs(&runtime_inputs).expect("runtime inputs");

        let evaluation = node
            .evaluate(&evaluation_context(), inputs)
            .expect("evaluate disabled node");

        assert!(evaluation.diagnostics.iter().any(|diagnostic| {
            diagnostic.code.as_deref() == Some("wled_target_missing_target")
                && diagnostic.severity == NodeDiagnosticSeverity::Warning
        }));
    }

    #[test]
    fn disabled_target_skips_invalid_value_types_for_disable_by_using_default() {
        let inputs =
            WledTargetInputs::from_runtime_inputs(&HashMap::new()).expect("default inputs");
        assert!(!is_disabled(inputs.disable));
    }
}
