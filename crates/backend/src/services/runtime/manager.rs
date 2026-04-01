use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anyhow::Context;
use shared::{GraphRuntimeMode, GraphRuntimeStatus};
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio::time::MissedTickBehavior;

use crate::node_runtime::NodeRegistry;
use crate::services::graph_store::GraphStore;
use crate::services::runtime::compiler::compile_graph_document;
use crate::services::runtime::types::{
    CompiledGraph, GraphExecutionState, GraphRuntimeCommand, RuntimeEventPublisher,
    RuntimeStatusesUpdate, RuntimeTask,
};

pub(crate) struct GraphRuntimeManager {
    node_registry: Arc<NodeRegistry>,
    graph_store: Arc<GraphStore>,
    events: Arc<dyn RuntimeEventPublisher>,
    state_path: PathBuf,
    tasks: Mutex<HashMap<String, RuntimeTask>>,
}

impl GraphRuntimeManager {
    /// Creates a runtime manager rooted at `root_dir`.
    ///
    /// The manager persists the set of running graphs beneath that root and uses the provided
    /// graph store, node registry, and event publisher to manage graph execution tasks.
    pub(crate) fn new(
        root_dir: &Path,
        node_registry: Arc<NodeRegistry>,
        graph_store: Arc<GraphStore>,
        events: Arc<dyn RuntimeEventPublisher>,
    ) -> Self {
        Self {
            node_registry,
            graph_store,
            events,
            state_path: root_dir.join("running_graphs.json"),
            tasks: Mutex::new(HashMap::new()),
        }
    }

    /// Restores persisted running graphs and respawns their execution tasks.
    ///
    /// Missing graph documents are skipped, duplicate graph IDs are removed, and graphs that fail
    /// to compile on startup are logged and left inactive.
    pub(crate) async fn load_persisted_state(self: &Arc<Self>) -> anyhow::Result<()> {
        let loaded = self.read_running_graph_ids().await?;
        let mut filtered = Vec::new();
        for graph_id in loaded {
            if self
                .graph_store
                .get_graph_document(&graph_id)
                .await?
                .is_some()
            {
                filtered.push(graph_id);
            }
        }
        filtered.sort();
        filtered.dedup();

        {
            let mut tasks = self.tasks.lock().await;
            tasks.clear();
        }

        let mut boot_started = Vec::new();
        for graph_id in &filtered {
            if let Some(document) = self.graph_store.get_graph_document(graph_id).await? {
                match compile_graph_document(document, self.node_registry.clone()) {
                    Ok(compiled) => {
                        emit_construction_diagnostics(&graph_id, &compiled, self.events.as_ref());
                        self.spawn_execution_task(
                            graph_id.to_owned(),
                            compiled,
                            GraphRuntimeMode::Running,
                        )
                        .await;
                        boot_started.push(graph_id.clone());
                    }
                    Err(error) => {
                        tracing::error!(graph_id, %error, "failed to compile graph on boot");
                    }
                }
            }
        }
        self.write_running_graph_ids(&boot_started).await
    }

    /// Returns the runtime status for every active graph task.
    pub(crate) async fn runtime_statuses(&self) -> Vec<GraphRuntimeStatus> {
        let tasks = self.tasks.lock().await;
        sorted_statuses(&tasks)
    }

    /// Starts or resumes execution for the given graph.
    ///
    /// If no execution task exists yet, the graph is loaded, compiled, and spawned before the
    /// start command is sent.
    pub(crate) async fn start_graph(
        self: &Arc<Self>,
        id: &str,
    ) -> anyhow::Result<RuntimeStatusesUpdate> {
        let id = id.trim();
        anyhow::ensure!(!id.is_empty(), "Graph document id must not be empty");
        self.ensure_execution_task(id, GraphRuntimeMode::Running)
            .await?;
        self.update_task_mode(id, GraphRuntimeMode::Running).await?;
        self.send_runtime_command(id, GraphRuntimeCommand::Start)
            .await?;
        let statuses = self.persist_and_emit_statuses().await?;
        Ok(RuntimeStatusesUpdate { statuses })
    }

    /// Pauses an active graph execution task.
    pub(crate) async fn pause_graph(&self, id: &str) -> anyhow::Result<RuntimeStatusesUpdate> {
        let id = id.trim();
        anyhow::ensure!(!id.is_empty(), "Graph document id must not be empty");

        self.update_task_mode(id, GraphRuntimeMode::Paused).await?;
        self.send_runtime_command(id, GraphRuntimeCommand::Pause)
            .await?;
        let statuses = self.persist_and_emit_statuses().await?;
        Ok(RuntimeStatusesUpdate { statuses })
    }

    /// Steps a graph forward by `ticks` frames while keeping it paused afterward.
    ///
    /// The graph is compiled and spawned first when no execution task exists yet.
    pub(crate) async fn step_graph(
        self: &Arc<Self>,
        id: &str,
        ticks: usize,
    ) -> anyhow::Result<RuntimeStatusesUpdate> {
        let id = id.trim();
        anyhow::ensure!(!id.is_empty(), "Graph document id must not be empty");
        anyhow::ensure!(ticks > 0, "Tick count must be greater than zero");

        self.ensure_execution_task(id, GraphRuntimeMode::Paused)
            .await?;
        self.update_task_mode(id, GraphRuntimeMode::Paused).await?;
        let (done_tx, done_rx) = oneshot::channel();
        self.send_runtime_command(id, GraphRuntimeCommand::Step { ticks, done_tx })
            .await?;
        let _ = done_rx.await;
        let statuses = self.persist_and_emit_statuses().await?;
        Ok(RuntimeStatusesUpdate { statuses })
    }

    /// Stops the execution task for the given graph.
    pub(crate) async fn stop_graph(&self, id: &str) -> anyhow::Result<RuntimeStatusesUpdate> {
        let id = id.trim();
        anyhow::ensure!(!id.is_empty(), "Graph document id must not be empty");

        self.stop_execution_task(id).await?;
        let statuses = self.persist_and_emit_statuses().await?;
        Ok(RuntimeStatusesUpdate { statuses })
    }

    /// Removes any runtime state associated with a graph.
    ///
    /// This currently delegates to [`Self::stop_graph`], because removing a graph from runtime is
    /// equivalent to stopping its execution task.
    pub(crate) async fn remove_graph(&self, id: &str) -> anyhow::Result<RuntimeStatusesUpdate> {
        self.stop_graph(id).await
    }

    /// Reads the persisted list of graph IDs that should resume in the running state.
    ///
    /// Returns an empty list when the persistence file does not exist yet.
    async fn read_running_graph_ids(&self) -> anyhow::Result<Vec<String>> {
        let payload = match tokio::fs::read_to_string(&self.state_path).await {
            Ok(payload) => payload,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => {
                return Err(error).with_context(|| format!("read {}", self.state_path.display()));
            }
        };

        serde_json::from_str::<Vec<String>>(&payload)
            .with_context(|| format!("parse {}", self.state_path.display()))
    }

    /// Persists the set of graph IDs that should resume in the running state.
    async fn write_running_graph_ids(&self, graph_ids: &[String]) -> anyhow::Result<()> {
        if let Some(parent) = self.state_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("create {}", parent.display()))?;
        }

        let payload =
            serde_json::to_vec_pretty(graph_ids).context("serialize running graph ids")?;
        tokio::fs::write(&self.state_path, payload)
            .await
            .with_context(|| format!("write {}", self.state_path.display()))
    }

    /// Ensures that an execution task exists for `graph_id`.
    ///
    /// Missing tasks cause the graph document to be loaded and compiled before a new task is
    /// spawned in `initial_mode`.
    async fn ensure_execution_task(
        self: &Arc<Self>,
        graph_id: &str,
        initial_mode: GraphRuntimeMode,
    ) -> anyhow::Result<()> {
        let graph_id = graph_id.trim();
        anyhow::ensure!(!graph_id.is_empty(), "Graph document id must not be empty");

        let already_running = {
            let tasks = self.tasks.lock().await;
            tasks.contains_key(graph_id)
        };
        if already_running {
            return Ok(());
        }

        let document = self
            .graph_store
            .get_graph_document(graph_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Graph document {graph_id} does not exist"))?;
        let compiled = compile_graph_document(document, self.node_registry.clone())?;
        emit_construction_diagnostics(graph_id, &compiled, self.events.as_ref());
        self.spawn_execution_task(graph_id.to_owned(), compiled, initial_mode)
            .await;
        Ok(())
    }

    /// Spawns the asynchronous execution task for a compiled graph.
    ///
    /// The task owns the compiled graph, ticks it according to its execution frequency, and
    /// reacts to runtime commands until it is stopped.
    async fn spawn_execution_task(
        self: &Arc<Self>,
        graph_id: String,
        compiled: CompiledGraph,
        initial_mode: GraphRuntimeMode,
    ) {
        let mut tasks = self.tasks.lock().await;
        if tasks.contains_key(&graph_id) {
            return;
        }

        let (stop_tx, mut stop_rx) = oneshot::channel::<()>();
        let (command_tx, mut command_rx) = mpsc::unbounded_channel::<GraphRuntimeCommand>();
        let graph_id_for_task = graph_id.clone();
        let events = self.events.clone();
        let manager = Arc::downgrade(self);
        tokio::spawn(async move {
            let mut compiled = compiled;
            let tick_hz = compiled.execution_frequency_hz.max(1);
            let period = Duration::from_secs_f64(1.0 / tick_hz as f64);
            let mut ticker = tokio::time::interval(period);
            ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
            let mut execution_state = GraphExecutionState::default();
            let mut mode = initial_mode;
            let mut tick_index = 0u64;

            loop {
                match mode {
                    GraphRuntimeMode::Running => {
                        tokio::select! {
                            _ = ticker.tick() => {
                                execute_graph_tick(
                                    &mut compiled,
                                    &graph_id_for_task,
                                    events.as_ref(),
                                    &mut execution_state,
                                    &mut tick_index,
                                    tick_hz,
                                );
                            }
                            Some(command) = command_rx.recv() => {
                                handle_runtime_command(
                                    command,
                                    &mut mode,
                                    &mut compiled,
                                    &graph_id_for_task,
                                    events.as_ref(),
                                    &mut execution_state,
                                    &mut tick_index,
                                    tick_hz,
                                );
                            }
                            _ = &mut stop_rx => {
                                break;
                            }
                        }
                    }
                    GraphRuntimeMode::Paused => {
                        tokio::select! {
                            Some(command) = command_rx.recv() => {
                                handle_runtime_command(
                                    command,
                                    &mut mode,
                                    &mut compiled,
                                    &graph_id_for_task,
                                    events.as_ref(),
                                    &mut execution_state,
                                    &mut tick_index,
                                    tick_hz,
                                );
                            }
                            _ = &mut stop_rx => {
                                break;
                            }
                        }
                    }
                }
            }
            tracing::info!(graph_id = %graph_id_for_task, "graph execution task stopped");

            if let Some(manager) = manager.upgrade() {
                if let Err(error) = manager.handle_task_exit(&graph_id_for_task).await {
                    tracing::warn!(graph_id = %graph_id_for_task, %error, "failed to reconcile graph execution task exit");
                }
            }
        });

        tasks.insert(
            graph_id,
            RuntimeTask {
                mode: initial_mode,
                stop_tx,
                command_tx,
            },
        );
    }

    /// Stops and removes the execution task for `graph_id`.
    async fn stop_execution_task(&self, graph_id: &str) -> anyhow::Result<()> {
        let mut tasks = self.tasks.lock().await;
        let task = tasks
            .remove(graph_id)
            .ok_or_else(|| anyhow::anyhow!("Graph document {graph_id} is not active"))?;
        let _ = task.stop_tx.send(());
        Ok(())
    }

    /// Sends a runtime command to an active execution task.
    async fn send_runtime_command(
        &self,
        graph_id: &str,
        command: GraphRuntimeCommand,
    ) -> anyhow::Result<()> {
        let tasks = self.tasks.lock().await;
        let task = tasks
            .get(graph_id)
            .ok_or_else(|| anyhow::anyhow!("Graph document {graph_id} is not active"))?;
        task.command_tx
            .send(command)
            .map_err(|_| anyhow::anyhow!("Graph execution task for {graph_id} is unavailable"))
    }

    /// Updates the cached mode stored for an active execution task.
    async fn update_task_mode(&self, graph_id: &str, mode: GraphRuntimeMode) -> anyhow::Result<()> {
        let mut tasks = self.tasks.lock().await;
        let task = tasks
            .get_mut(graph_id)
            .ok_or_else(|| anyhow::anyhow!("Graph document {graph_id} is not active"))?;
        task.mode = mode;
        Ok(())
    }

    /// Reconciles manager state after an execution task exits on its own.
    async fn handle_task_exit(&self, graph_id: &str) -> anyhow::Result<()> {
        let removed = {
            let mut tasks = self.tasks.lock().await;
            tasks.remove(graph_id).is_some()
        };

        if removed {
            let statuses = self.persist_and_emit_statuses().await?;
            tracing::info!(
                graph_id,
                remaining_graphs = statuses.len(),
                "reconciled graph execution task exit"
            );
        }

        Ok(())
    }

    /// Persists runtime status and broadcasts the latest status list.
    ///
    /// Only graphs currently in the `Running` mode are written to the persisted state file.
    async fn persist_and_emit_statuses(&self) -> anyhow::Result<Vec<GraphRuntimeStatus>> {
        let statuses = self.runtime_statuses().await;
        let running_ids = statuses
            .iter()
            .filter(|status| status.mode == GraphRuntimeMode::Running)
            .map(|status| status.graph_id.clone())
            .collect::<Vec<_>>();
        self.write_running_graph_ids(&running_ids).await?;
        self.events.runtime_statuses_changed(statuses.clone());
        Ok(statuses)
    }
}

/// Executes a single graph tick and advances the tick counter.
///
/// Tick failures are logged and do not stop the surrounding execution task.
fn execute_graph_tick(
    compiled: &mut CompiledGraph,
    graph_id: &str,
    events: &dyn RuntimeEventPublisher,
    execution_state: &mut GraphExecutionState,
    tick_index: &mut u64,
    tick_hz: u32,
) {
    let elapsed_seconds = *tick_index as f64 / tick_hz as f64;
    if let Err(error) = compiled.execute_tick(graph_id, events, elapsed_seconds, execution_state) {
        tracing::warn!(graph_id, %error, "graph execution tick failed");
    }
    *tick_index = tick_index.saturating_add(1);
}

/// Applies a runtime command to an active execution task.
///
/// Step commands execute ticks immediately and then notify the caller through `done_tx`.
fn handle_runtime_command(
    command: GraphRuntimeCommand,
    mode: &mut GraphRuntimeMode,
    compiled: &mut CompiledGraph,
    graph_id: &str,
    events: &dyn RuntimeEventPublisher,
    execution_state: &mut GraphExecutionState,
    tick_index: &mut u64,
    tick_hz: u32,
) {
    match command {
        GraphRuntimeCommand::Start => *mode = GraphRuntimeMode::Running,
        GraphRuntimeCommand::Pause => *mode = GraphRuntimeMode::Paused,
        GraphRuntimeCommand::Step { ticks, done_tx } => {
            *mode = GraphRuntimeMode::Paused;
            for _ in 0..ticks {
                execute_graph_tick(
                    compiled,
                    graph_id,
                    events,
                    execution_state,
                    tick_index,
                    tick_hz,
                );
            }
            let _ = done_tx.send(());
        }
    }
}

/// Emits any construction diagnostics produced while compiling a graph.
fn emit_construction_diagnostics(
    graph_id: &str,
    compiled: &CompiledGraph,
    events: &dyn RuntimeEventPublisher,
) {
    for node in &compiled.nodes {
        if node.construction_diagnostics.is_empty() {
            continue;
        }
        events.node_diagnostics(
            graph_id.to_owned(),
            node.id.clone(),
            node.construction_diagnostics.clone(),
        );
    }
}

/// Returns runtime statuses sorted by graph ID.
fn sorted_statuses(tasks: &HashMap<String, RuntimeTask>) -> Vec<GraphRuntimeStatus> {
    let mut statuses = tasks
        .iter()
        .map(|(graph_id, task)| GraphRuntimeStatus {
            graph_id: graph_id.clone(),
            mode: task.mode,
        })
        .collect::<Vec<_>>();
    statuses.sort_by(|a, b| a.graph_id.cmp(&b.graph_id));
    statuses
}
