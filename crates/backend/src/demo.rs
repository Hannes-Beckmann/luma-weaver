use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use anyhow::Context;
use futures_channel::mpsc;
use futures_channel::mpsc::TryRecvError;
use shared::{
    ClientMessage, EventSubscription, GraphDocument, GraphExchangeFile, GraphMetadata,
    GraphRuntimeMode, GraphRuntimeStatus, MqttBrokerConfig, NodeDiagnostic, NodeDiagnosticEntry,
    NodeDiagnosticSeverity, NodeDiagnosticSummary, NodeExecutionTarget, NodeRuntimeUpdateValue,
    NodeRuntimeValue, NodeSchema, ServerMessage, ServerState, WledInstance, node_definitions,
    validate_graph_document,
};
use uuid::Uuid;

use crate::node_runtime::build_portable_node_registry;
use crate::services::runtime::compiler::compile_graph_document;
use crate::services::runtime::types::{CompiledGraph, GraphExecutionState, RuntimeEventPublisher};

const DEMO_STATUS: &str = "demo mode: local runtime";

pub fn connect_demo() -> anyhow::Result<(
    mpsc::UnboundedSender<ClientMessage>,
    mpsc::UnboundedReceiver<ServerMessage>,
    DemoTransport,
)> {
    let (request_tx, request_rx) = mpsc::unbounded();
    let (response_tx, response_rx) = mpsc::unbounded();
    let mut transport = DemoTransport::new(request_rx, response_tx)?;
    transport.push(ServerMessage::Welcome {
        message: "Demo mode: local runtime, no backend".to_owned(),
    });
    transport.push(ServerMessage::State(transport.server_state()));
    Ok((request_tx, response_rx, transport))
}

pub struct DemoTransport {
    request_rx: mpsc::UnboundedReceiver<ClientMessage>,
    response_tx: mpsc::UnboundedSender<ServerMessage>,
    node_definitions: Vec<NodeSchema>,
    node_registry: Arc<crate::node_runtime::NodeRegistry>,
    graph_documents: Vec<GraphDocument>,
    event_subscriptions: HashSet<EventSubscription>,
    runtime_graph_subscriptions: HashSet<String>,
    diagnostics_graph_subscriptions: HashSet<String>,
    node_diagnostics_subscriptions: HashSet<(String, String)>,
    runtimes: HashMap<String, DemoRuntimeGraph>,
    mqtt_broker_configs: Vec<MqttBrokerConfig>,
}

struct DemoRuntimeGraph {
    compiled: CompiledGraph,
    execution_state: GraphExecutionState,
    mode: GraphRuntimeMode,
    elapsed_seconds: f64,
    last_wallclock_seconds: Option<f64>,
    diagnostics_by_node: HashMap<String, Vec<NodeDiagnostic>>,
}

impl DemoTransport {
    fn new(
        request_rx: mpsc::UnboundedReceiver<ClientMessage>,
        response_tx: mpsc::UnboundedSender<ServerMessage>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            request_rx,
            response_tx,
            node_registry: build_portable_node_registry()
                .context("build portable node registry")?,
            node_definitions: node_definitions()
                .into_iter()
                .filter(|definition| {
                    definition.supports_execution_target(NodeExecutionTarget::FrontendDemo)
                })
                .collect(),
            graph_documents: seeded_demo_graphs()?,
            event_subscriptions: HashSet::new(),
            runtime_graph_subscriptions: HashSet::new(),
            diagnostics_graph_subscriptions: HashSet::new(),
            node_diagnostics_subscriptions: HashSet::new(),
            runtimes: HashMap::new(),
            mqtt_broker_configs: Vec::new(),
        })
    }

    pub fn pump(&mut self, now_secs: f64) {
        loop {
            match self.request_rx.try_recv() {
                Ok(message) => self.handle_client_message(message, now_secs),
                Err(TryRecvError::Empty) | Err(TryRecvError::Closed) => break,
            }
        }

        self.tick_running_graphs(now_secs);
    }

    fn handle_client_message(&mut self, message: ClientMessage, now_secs: f64) {
        let response = match message {
            ClientMessage::Ping => Some(ServerMessage::Pong),
            ClientMessage::SetName { .. } => None,
            ClientMessage::Subscribe { subscriptions } => {
                self.event_subscriptions.extend(subscriptions);
                Some(ServerMessage::SubscriptionState {
                    subscriptions: sorted_subscriptions(&self.event_subscriptions),
                })
            }
            ClientMessage::Unsubscribe { subscriptions } => {
                for subscription in subscriptions {
                    self.event_subscriptions.remove(&subscription);
                }
                Some(ServerMessage::SubscriptionState {
                    subscriptions: sorted_subscriptions(&self.event_subscriptions),
                })
            }
            ClientMessage::CreateGraphDocument { name } => self.create_graph_document(name),
            ClientMessage::DeleteGraphDocument { id } => self.delete_graph_document(&id),
            ClientMessage::GetGraphDocument { id } => self.get_graph_document(&id),
            ClientMessage::UpdateGraphDocument { document } => self.update_graph_document(document),
            ClientMessage::UpdateGraphName { id, name } => self.update_graph_name(&id, name),
            ClientMessage::UpdateGraphExecutionFrequency {
                id,
                execution_frequency_hz,
            } => self.update_graph_execution_frequency(&id, execution_frequency_hz),
            ClientMessage::GetNodeDefinitions => Some(ServerMessage::NodeDefinitions {
                definitions: self.node_definitions.clone(),
            }),
            ClientMessage::GetGraphMetadata => Some(ServerMessage::GraphMetadata {
                documents: self.graph_metadata(),
            }),
            ClientMessage::StartGraph { id } => self.start_graph(&id, now_secs),
            ClientMessage::PauseGraph { id } => self.pause_graph(&id),
            ClientMessage::StepGraph { id, ticks } => self.step_graph(&id, ticks as usize),
            ClientMessage::StopGraph { id } => self.stop_graph(&id),
            ClientMessage::GetRuntimeStatuses => Some(ServerMessage::RuntimeStatuses {
                graphs: self.runtime_statuses(),
            }),
            ClientMessage::SubscribeGraphRuntime { graph_id } => {
                self.runtime_graph_subscriptions
                    .insert(graph_id.trim().to_owned());
                None
            }
            ClientMessage::UnsubscribeGraphRuntime { graph_id } => {
                self.runtime_graph_subscriptions.remove(graph_id.trim());
                None
            }
            ClientMessage::SubscribeGraphDiagnostics { graph_id } => {
                let graph_id = graph_id.trim().to_owned();
                self.diagnostics_graph_subscriptions
                    .insert(graph_id.clone());
                self.push_graph_diagnostics_summary(&graph_id);
                None
            }
            ClientMessage::UnsubscribeGraphDiagnostics { graph_id } => {
                self.diagnostics_graph_subscriptions.remove(graph_id.trim());
                None
            }
            ClientMessage::SubscribeNodeDiagnostics { graph_id, node_id } => {
                let key = (graph_id.trim().to_owned(), node_id.trim().to_owned());
                self.node_diagnostics_subscriptions.insert(key.clone());
                self.push_node_diagnostics_detail(&key.0, &key.1);
                None
            }
            ClientMessage::UnsubscribeNodeDiagnostics { graph_id, node_id } => {
                self.node_diagnostics_subscriptions
                    .remove(&(graph_id.trim().to_owned(), node_id.trim().to_owned()));
                None
            }
            ClientMessage::ClearNodeDiagnostics { graph_id, node_id } => {
                if let Some(runtime) = self.runtimes.get_mut(graph_id.trim()) {
                    runtime.diagnostics_by_node.remove(node_id.trim());
                    self.push_graph_diagnostics_summary(graph_id.trim());
                    self.push_node_diagnostics_detail(graph_id.trim(), node_id.trim());
                }
                None
            }
            ClientMessage::GetWledInstances => Some(ServerMessage::WledInstances {
                instances: Vec::<WledInstance>::new(),
            }),
            ClientMessage::GetMqttBrokerConfigs => Some(ServerMessage::MqttBrokerConfigs {
                brokers: self.mqtt_broker_configs.clone(),
            }),
            ClientMessage::UpdateMqttBrokerConfigs { brokers } => {
                self.mqtt_broker_configs = brokers.clone();
                Some(ServerMessage::MqttBrokerConfigs { brokers })
            }
            ClientMessage::ExportGraphDocument { .. }
            | ClientMessage::ImportGraphDocument { .. } => Some(ServerMessage::Error {
                message: "Import and export are not available in demo mode yet".to_owned(),
            }),
        };

        if let Some(response) = response {
            self.push(response);
        }
        self.push(ServerMessage::State(self.server_state()));
    }

    fn create_graph_document(&mut self, name: String) -> Option<ServerMessage> {
        let trimmed_name = name.trim().to_owned();
        if trimmed_name.is_empty() {
            return Some(ServerMessage::Error {
                message: "Graph document name must not be empty".to_owned(),
            });
        }

        self.graph_documents.push(GraphDocument {
            metadata: GraphMetadata {
                id: Uuid::new_v4().to_string(),
                name: trimmed_name,
                execution_frequency_hz: 60,
            },
            viewport: shared::GraphViewport::default(),
            nodes: Vec::new(),
            edges: Vec::new(),
        });
        self.sort_graph_documents();
        Some(ServerMessage::GraphMetadata {
            documents: self.graph_metadata(),
        })
    }

    fn delete_graph_document(&mut self, id: &str) -> Option<ServerMessage> {
        let id = id.trim();
        if id.is_empty() {
            return Some(ServerMessage::Error {
                message: "Graph document id must not be empty".to_owned(),
            });
        }

        let previous_len = self.graph_documents.len();
        self.graph_documents
            .retain(|document| document.metadata.id != id);
        self.runtimes.remove(id);
        if self.graph_documents.len() == previous_len {
            return Some(ServerMessage::Error {
                message: format!("Graph document {id} was not found"),
            });
        }
        Some(ServerMessage::GraphMetadata {
            documents: self.graph_metadata(),
        })
    }

    fn get_graph_document(&self, id: &str) -> Option<ServerMessage> {
        let id = id.trim();
        if id.is_empty() {
            return Some(ServerMessage::Error {
                message: "Graph document id must not be empty".to_owned(),
            });
        }

        self.graph_documents
            .iter()
            .find(|document| document.metadata.id == id)
            .cloned()
            .map(|document| ServerMessage::GraphDocument { document })
            .or_else(|| {
                Some(ServerMessage::Error {
                    message: format!("Graph document {id} was not found"),
                })
            })
    }

    fn update_graph_document(&mut self, document: GraphDocument) -> Option<ServerMessage> {
        if let Err(error) = validate_demo_document(&document) {
            return Some(ServerMessage::Error {
                message: error.to_string(),
            });
        }

        let graph_id = document.metadata.id.clone();
        match self
            .graph_documents
            .iter_mut()
            .find(|existing| existing.metadata.id == graph_id)
        {
            Some(existing) => *existing = document.clone(),
            None => self.graph_documents.push(document.clone()),
        }
        self.sort_graph_documents();
        Some(ServerMessage::GraphDocument { document })
    }

    fn update_graph_name(&mut self, id: &str, name: String) -> Option<ServerMessage> {
        let id = id.trim();
        let trimmed_name = name.trim().to_owned();
        if id.is_empty() {
            return Some(ServerMessage::Error {
                message: "Graph document id must not be empty".to_owned(),
            });
        }
        if trimmed_name.is_empty() {
            return Some(ServerMessage::Error {
                message: "Graph document name must not be empty".to_owned(),
            });
        }

        {
            let Some(document) = self
                .graph_documents
                .iter_mut()
                .find(|document| document.metadata.id == id)
            else {
                return Some(ServerMessage::Error {
                    message: format!("Graph document {id} was not found"),
                });
            };
            document.metadata.name = trimmed_name;
        }
        self.sort_graph_documents();
        Some(ServerMessage::GraphMetadata {
            documents: self.graph_metadata(),
        })
    }

    fn update_graph_execution_frequency(
        &mut self,
        id: &str,
        execution_frequency_hz: u32,
    ) -> Option<ServerMessage> {
        let id = id.trim();
        {
            let Some(document) = self
                .graph_documents
                .iter_mut()
                .find(|document| document.metadata.id == id)
            else {
                return Some(ServerMessage::Error {
                    message: format!("Graph document {id} was not found"),
                });
            };

            document.metadata.execution_frequency_hz = execution_frequency_hz.max(1);
        }
        Some(ServerMessage::GraphMetadata {
            documents: self.graph_metadata(),
        })
    }

    fn start_graph(&mut self, id: &str, now_secs: f64) -> Option<ServerMessage> {
        let id = id.trim();
        if let Err(error) = self.replace_runtime(id) {
            return Some(ServerMessage::Error {
                message: error.to_string(),
            });
        }
        if let Some(runtime) = self.runtimes.get_mut(id) {
            runtime.mode = GraphRuntimeMode::Running;
            runtime.last_wallclock_seconds = Some(now_secs);
        }
        Some(ServerMessage::RuntimeStatuses {
            graphs: self.runtime_statuses(),
        })
    }

    fn pause_graph(&mut self, id: &str) -> Option<ServerMessage> {
        let id = id.trim();
        let Some(runtime) = self.runtimes.get_mut(id) else {
            return Some(ServerMessage::Error {
                message: format!("Graph runtime {id} is not active"),
            });
        };
        runtime.mode = GraphRuntimeMode::Paused;
        runtime.last_wallclock_seconds = None;
        Some(ServerMessage::RuntimeStatuses {
            graphs: self.runtime_statuses(),
        })
    }

    fn step_graph(&mut self, id: &str, ticks: usize) -> Option<ServerMessage> {
        let id = id.trim();
        if ticks == 0 {
            return Some(ServerMessage::Error {
                message: "Tick count must be greater than zero".to_owned(),
            });
        }
        if let Err(error) = self.ensure_runtime(id) {
            return Some(ServerMessage::Error {
                message: error.to_string(),
            });
        }
        if let Some(runtime) = self.runtimes.get_mut(id) {
            runtime.mode = GraphRuntimeMode::Paused;
            runtime.last_wallclock_seconds = None;
        }
        if let Err(error) = self.execute_ticks(id, ticks) {
            return Some(ServerMessage::Error {
                message: error.to_string(),
            });
        }
        Some(ServerMessage::RuntimeStatuses {
            graphs: self.runtime_statuses(),
        })
    }

    fn stop_graph(&mut self, id: &str) -> Option<ServerMessage> {
        let id = id.trim();
        if self.runtimes.remove(id).is_none() {
            return Some(ServerMessage::Error {
                message: format!("Graph runtime {id} is not active"),
            });
        }
        Some(ServerMessage::RuntimeStatuses {
            graphs: self.runtime_statuses(),
        })
    }

    fn tick_running_graphs(&mut self, now_secs: f64) {
        let running_ids = self
            .runtimes
            .iter()
            .filter_map(|(graph_id, runtime)| {
                (runtime.mode == GraphRuntimeMode::Running).then_some(graph_id.clone())
            })
            .collect::<Vec<_>>();

        for graph_id in running_ids {
            let ticks = {
                let Some(runtime) = self.runtimes.get_mut(&graph_id) else {
                    continue;
                };
                let tick_interval = 1.0 / runtime.compiled.execution_frequency_hz.max(1) as f64;
                let mut ticks = 0usize;
                let mut last = runtime.last_wallclock_seconds.unwrap_or(now_secs);
                while now_secs - last >= tick_interval && ticks < 8 {
                    last += tick_interval;
                    ticks += 1;
                }
                runtime.last_wallclock_seconds = Some(last);
                ticks
            };
            if ticks > 0 {
                let _ = self.execute_ticks(&graph_id, ticks);
            }
        }
    }

    fn ensure_runtime(&mut self, graph_id: &str) -> anyhow::Result<()> {
        if self.runtimes.contains_key(graph_id) {
            return Ok(());
        }
        let document = self
            .graph_documents
            .iter()
            .find(|document| document.metadata.id == graph_id)
            .cloned()
            .with_context(|| format!("Graph document {graph_id} was not found"))?;
        let compiled = compile_graph_document(document, self.node_registry.clone())
            .with_context(|| format!("compile graph {graph_id}"))?;
        let diagnostics_by_node = compiled
            .nodes
            .iter()
            .filter(|node| !node.construction_diagnostics.is_empty())
            .map(|node| (node.id.clone(), node.construction_diagnostics.clone()))
            .collect();
        self.runtimes.insert(
            graph_id.to_owned(),
            DemoRuntimeGraph {
                compiled,
                execution_state: GraphExecutionState::default(),
                mode: GraphRuntimeMode::Paused,
                elapsed_seconds: 0.0,
                last_wallclock_seconds: None,
                diagnostics_by_node,
            },
        );
        self.push_graph_diagnostics_summary(graph_id);
        Ok(())
    }

    fn replace_runtime(&mut self, graph_id: &str) -> anyhow::Result<()> {
        self.runtimes.remove(graph_id);
        self.ensure_runtime(graph_id)?;
        Ok(())
    }

    fn execute_ticks(&mut self, graph_id: &str, ticks: usize) -> anyhow::Result<()> {
        let runtime_subscribed = self.runtime_graph_subscriptions.contains(graph_id);
        let diagnostics_graph_subscribed = self.diagnostics_graph_subscriptions.contains(graph_id);
        let node_diagnostics_subscriptions = self.node_diagnostics_subscriptions.clone();

        for _ in 0..ticks {
            let (runtime_updates, diagnostics_changed, detail_messages) = {
                let Some(runtime) = self.runtimes.get_mut(graph_id) else {
                    anyhow::bail!("Graph runtime {graph_id} is not active");
                };
                let tick_interval = 1.0 / runtime.compiled.execution_frequency_hz.max(1) as f64;
                runtime.elapsed_seconds += tick_interval;
                let collector = Mutex::new(DemoEventCollector::new(
                    graph_id,
                    runtime_subscribed,
                    diagnostics_graph_subscribed,
                    node_diagnostics_subscriptions.clone(),
                ));
                runtime.compiled.execute_tick(
                    graph_id,
                    &collector,
                    runtime.elapsed_seconds,
                    &mut runtime.execution_state,
                )?;
                let collector = collector.into_inner().expect("demo collector mutex");

                let mut detail_messages = Vec::new();
                let diagnostics_changed = !collector.node_diagnostics.is_empty();
                for (node_id, diagnostics) in &collector.node_diagnostics {
                    runtime
                        .diagnostics_by_node
                        .insert(node_id.clone(), diagnostics.clone());
                    if node_diagnostics_subscriptions
                        .contains(&(graph_id.to_owned(), node_id.clone()))
                    {
                        detail_messages.push(ServerMessage::NodeDiagnosticsDetail {
                            graph_id: graph_id.to_owned(),
                            node_id: node_id.clone(),
                            diagnostics: aggregate_diagnostics(diagnostics),
                        });
                    }
                }

                (
                    collector.runtime_updates,
                    diagnostics_changed,
                    detail_messages,
                )
            };

            if diagnostics_graph_subscribed && diagnostics_changed {
                self.push_graph_diagnostics_summary(graph_id);
            }
            for message in detail_messages {
                self.push(message);
            }
            for message in runtime_updates {
                self.push(message);
            }
        }

        Ok(())
    }

    fn runtime_statuses(&self) -> Vec<GraphRuntimeStatus> {
        let mut statuses = self
            .runtimes
            .iter()
            .map(|(graph_id, runtime)| GraphRuntimeStatus {
                graph_id: graph_id.clone(),
                mode: runtime.mode,
            })
            .collect::<Vec<_>>();
        statuses.sort_by(|left, right| left.graph_id.cmp(&right.graph_id));
        statuses
    }

    fn graph_metadata(&self) -> Vec<GraphMetadata> {
        let mut metadata = self
            .graph_documents
            .iter()
            .map(|document| document.metadata.clone())
            .collect::<Vec<_>>();
        metadata.sort_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));
        metadata
    }

    fn push_graph_diagnostics_summary(&mut self, graph_id: &str) {
        if !self.diagnostics_graph_subscriptions.contains(graph_id) {
            return;
        }
        let nodes = self
            .runtimes
            .get(graph_id)
            .map(|runtime| diagnostics_summary(&runtime.diagnostics_by_node))
            .unwrap_or_default();
        self.push(ServerMessage::GraphDiagnosticsSummary {
            graph_id: graph_id.to_owned(),
            nodes,
        });
    }

    fn push_node_diagnostics_detail(&mut self, graph_id: &str, node_id: &str) {
        if !self
            .node_diagnostics_subscriptions
            .contains(&(graph_id.to_owned(), node_id.to_owned()))
        {
            return;
        }
        let diagnostics = self
            .runtimes
            .get(graph_id)
            .and_then(|runtime| runtime.diagnostics_by_node.get(node_id))
            .map(|diagnostics| aggregate_diagnostics(diagnostics))
            .unwrap_or_default();
        self.push(ServerMessage::NodeDiagnosticsDetail {
            graph_id: graph_id.to_owned(),
            node_id: node_id.to_owned(),
            diagnostics,
        });
    }

    fn sort_graph_documents(&mut self) {
        self.graph_documents.sort_by(|left, right| {
            left.metadata
                .name
                .cmp(&right.metadata.name)
                .then(left.metadata.id.cmp(&right.metadata.id))
        });
    }

    fn server_state(&self) -> ServerState {
        ServerState {
            connected_clients: 1,
            status: DEMO_STATUS.to_owned(),
        }
    }

    fn push(&mut self, message: ServerMessage) {
        let _ = self.response_tx.unbounded_send(message);
    }
}

struct DemoEventCollector {
    runtime_subscribed: bool,
    diagnostics_graph_subscribed: bool,
    node_diagnostics_subscriptions: HashSet<(String, String)>,
    runtime_updates: Vec<ServerMessage>,
    node_diagnostics: HashMap<String, Vec<NodeDiagnostic>>,
}

impl DemoEventCollector {
    fn new(
        _graph_id: &str,
        runtime_subscribed: bool,
        diagnostics_graph_subscribed: bool,
        node_diagnostics_subscriptions: HashSet<(String, String)>,
    ) -> Self {
        Self {
            runtime_subscribed,
            diagnostics_graph_subscribed,
            node_diagnostics_subscriptions,
            runtime_updates: Vec::new(),
            node_diagnostics: HashMap::new(),
        }
    }
}

impl RuntimeEventPublisher for Mutex<DemoEventCollector> {
    fn runtime_statuses_changed(&self, _statuses: Vec<GraphRuntimeStatus>) {}

    fn node_runtime_update(
        &self,
        graph_id: String,
        node_id: String,
        values: Vec<NodeRuntimeValue>,
    ) {
        let mut collector = self.lock().expect("demo collector mutex");
        if !collector.runtime_subscribed {
            return;
        }
        collector
            .runtime_updates
            .push(ServerMessage::NodeRuntimeUpdate {
                graph_id,
                node_id,
                values: values
                    .into_iter()
                    .map(|value| NodeRuntimeUpdateValue::Inline {
                        name: value.name,
                        value: value.value,
                    })
                    .collect(),
            });
    }

    fn node_diagnostics(
        &self,
        graph_id: String,
        node_id: String,
        diagnostics: Vec<NodeDiagnostic>,
    ) {
        let mut collector = self.lock().expect("demo collector mutex");
        if collector.diagnostics_graph_subscribed
            || collector
                .node_diagnostics_subscriptions
                .contains(&(graph_id, node_id.clone()))
        {
            collector.node_diagnostics.insert(node_id, diagnostics);
        }
    }
}

fn validate_demo_document(document: &GraphDocument) -> anyhow::Result<()> {
    let validation_issues = validate_graph_document(document);
    anyhow::ensure!(
        validation_issues.is_empty(),
        "{}",
        validation_issues
            .into_iter()
            .map(|issue| issue.message)
            .collect::<Vec<_>>()
            .join("; ")
    );
    for node in &document.nodes {
        let supported = node_definitions()
            .into_iter()
            .find(|definition| definition.id == node.node_type.as_str())
            .map(|definition| {
                definition.supports_execution_target(NodeExecutionTarget::FrontendDemo)
            })
            .unwrap_or(false);
        anyhow::ensure!(
            supported,
            "Node type {} is not available in demo mode",
            node.node_type.as_str()
        );
    }
    Ok(())
}

fn diagnostics_summary(
    diagnostics_by_node: &HashMap<String, Vec<NodeDiagnostic>>,
) -> Vec<NodeDiagnosticSummary> {
    let mut summaries = diagnostics_by_node
        .iter()
        .filter(|(_, diagnostics)| !diagnostics.is_empty())
        .map(|(node_id, diagnostics)| NodeDiagnosticSummary {
            node_id: node_id.clone(),
            highest_severity: diagnostics
                .iter()
                .map(|diagnostic| diagnostic.severity)
                .max()
                .unwrap_or(NodeDiagnosticSeverity::Info),
            active_count: diagnostics.len(),
        })
        .collect::<Vec<_>>();
    summaries.sort_by(|left, right| left.node_id.cmp(&right.node_id));
    summaries
}

fn aggregate_diagnostics(diagnostics: &[NodeDiagnostic]) -> Vec<NodeDiagnosticEntry> {
    diagnostics
        .iter()
        .map(|diagnostic| NodeDiagnosticEntry {
            severity: diagnostic.severity,
            code: diagnostic.code.clone(),
            message: diagnostic.message.clone(),
            occurrences: 1,
        })
        .collect()
}

fn sorted_subscriptions(subscriptions: &HashSet<EventSubscription>) -> Vec<EventSubscription> {
    let mut sorted = subscriptions.iter().cloned().collect::<Vec<_>>();
    sorted.sort_by(|left, right| format!("{left:?}").cmp(&format!("{right:?}")));
    sorted
}

fn seeded_demo_graphs() -> anyhow::Result<Vec<GraphDocument>> {
    let example = serde_json::from_str::<GraphExchangeFile>(include_str!(
        "../../../examples/Example.animation-graph.json"
    ))
    .context("parse seeded example graph")?
    .document;

    let signal_playground = serde_json::from_value::<GraphDocument>(serde_json::json!({
        "metadata": {
            "id": "demo-signal-playground",
            "name": "Signal Playground",
            "execution_frequency_hz": 60
        },
        "viewport": {
            "zoom": 1.0,
            "pan": { "x": 420.0, "y": 160.0 }
        },
        "nodes": [
            {
                "id": "signal_1",
                "metadata": { "name": "Signal Generator" },
                "node_type": "inputs.signal_generator",
                "viewport": {
                    "position": { "x": -540.0, "y": 80.0 },
                    "collapsed": false
                },
                "input_values": [],
                "parameters": [
                    { "name": "waveform", "value": "sinus" },
                    { "name": "frequency", "value": 0.35 },
                    { "name": "amplitude", "value": 0.8 },
                    { "name": "phase", "value": 0.0 },
                    { "name": "offset", "value": 0.0 }
                ]
            },
            {
                "id": "plot_1",
                "metadata": { "name": "Plot" },
                "node_type": "outputs.plot",
                "viewport": {
                    "position": { "x": -20.0, "y": -10.0 },
                    "collapsed": false
                },
                "input_values": [{ "name": "value", "value": { "kind": "Float", "value": 0.0 } }],
                "parameters": []
            }
        ],
        "edges": [
            {
                "from_node_id": "signal_1",
                "from_output_name": "value",
                "to_node_id": "plot_1",
                "to_input_name": "value"
            }
        ]
    }))
    .context("build seeded signal playground graph")?;

    Ok(vec![example, signal_playground])
}

#[cfg(test)]
mod tests {
    use futures_channel::mpsc;
    use shared::{ClientMessage, EventScope, EventSubscription, EventTopic, ServerMessage};

    use super::DemoTransport;

    #[test]
    fn subscribe_accumulates_and_unsubscribe_removes_subscriptions() {
        let (_request_tx, request_rx) = mpsc::unbounded();
        let (response_tx, _response_rx) = mpsc::unbounded();
        let mut transport = DemoTransport::new(request_rx, response_tx).expect("demo transport");

        let connection = EventSubscription {
            topic: EventTopic::Connection,
            scope: EventScope::Global,
        };
        let ping = EventSubscription {
            topic: EventTopic::Ping,
            scope: EventScope::Global,
        };

        transport.handle_client_message(
            ClientMessage::Subscribe {
                subscriptions: vec![connection.clone()],
            },
            0.0,
        );
        transport.handle_client_message(
            ClientMessage::Subscribe {
                subscriptions: vec![ping.clone()],
            },
            0.0,
        );

        assert!(transport.event_subscriptions.contains(&connection));
        assert!(transport.event_subscriptions.contains(&ping));

        transport.handle_client_message(
            ClientMessage::Unsubscribe {
                subscriptions: vec![connection.clone()],
            },
            0.0,
        );

        assert!(!transport.event_subscriptions.contains(&connection));
        assert!(transport.event_subscriptions.contains(&ping));
    }

    #[test]
    fn subscribe_response_contains_accumulated_subscriptions() {
        let (_request_tx, request_rx) = mpsc::unbounded();
        let (response_tx, mut response_rx) = mpsc::unbounded();
        let mut transport = DemoTransport::new(request_rx, response_tx).expect("demo transport");

        let connection = EventSubscription {
            topic: EventTopic::Connection,
            scope: EventScope::Global,
        };
        let ping = EventSubscription {
            topic: EventTopic::Ping,
            scope: EventScope::Global,
        };

        transport.handle_client_message(
            ClientMessage::Subscribe {
                subscriptions: vec![connection.clone()],
            },
            0.0,
        );
        let _ = response_rx.try_next();
        let _ = response_rx.try_next();

        transport.handle_client_message(
            ClientMessage::Subscribe {
                subscriptions: vec![ping.clone()],
            },
            0.0,
        );

        let message = response_rx
            .try_next()
            .expect("subscription state result")
            .expect("subscription state message");
        let ServerMessage::SubscriptionState { subscriptions } = message else {
            panic!("expected subscription state");
        };

        assert!(subscriptions.contains(&connection));
        assert!(subscriptions.contains(&ping));
    }
}
