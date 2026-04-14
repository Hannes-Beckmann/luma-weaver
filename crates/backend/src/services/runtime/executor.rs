use std::collections::HashMap;
use std::time::{Duration, Instant};

use anyhow::Context;
use shared::{InputValue, NodeDiagnostic, NodeDiagnosticSeverity, NodeRuntimeValue};

use crate::node_runtime::nodes::temporal_filters::delay::seeded_initial_value_for_layout;
use crate::node_runtime::{NodeEvaluation, NodeEvaluationContext, RuntimeNodeEvaluator};
use crate::services::runtime::types::{CompiledGraph, GraphExecutionState, RuntimeEventPublisher};

impl CompiledGraph {
    /// Executes one tick of the compiled graph.
    ///
    /// The graph is evaluated in topological order, reusing per-node evaluators across ticks and
    /// across render contexts. Runtime updates and diagnostics are emitted through `events`, and
    /// the produced outputs become the previous-tick values for the next invocation.
    pub(crate) fn execute_tick(
        &mut self,
        graph_id: &str,
        events: &dyn RuntimeEventPublisher,
        elapsed_seconds: f64,
        execution_state: &mut GraphExecutionState,
    ) -> anyhow::Result<()> {
        const DEFAULT_CONTEXT_ID: &str = "__default__";
        const RUNTIME_UPDATE_MIN_INTERVAL: Duration = Duration::from_millis(50);
        let tick_started_at = Instant::now();
        initialize_delay_previous_outputs(self, execution_state);
        let previous_outputs = execution_state.previous_outputs.clone();
        let mut outputs = HashMap::<(usize, String, String), InputValue>::new();

        let topological_order = self.topological_order.clone();
        for node_index in topological_order {
            let render_contexts =
                evaluation_contexts_for_node(&self.render_contexts_by_node[node_index]);
            let incoming_edges = self.incoming_edges_by_node[node_index].clone();
            let node = &mut self.nodes[node_index];

            for render_context in render_contexts {
                let context_id = render_context
                    .as_ref()
                    .map(|ctx| ctx.id.as_str())
                    .unwrap_or(DEFAULT_CONTEXT_ID);
                let mut inputs = node.input_defaults.clone();

                for incoming in &incoming_edges {
                    let context_key = (
                        incoming.from_node_index,
                        context_id.to_owned(),
                        incoming.from_output_name.clone(),
                    );
                    let fallback_key = (
                        incoming.from_node_index,
                        DEFAULT_CONTEXT_ID.to_owned(),
                        incoming.from_output_name.clone(),
                    );
                    let value = if incoming.use_previous_tick {
                        previous_outputs
                            .get(&context_key)
                            .cloned()
                            .or_else(|| previous_outputs.get(&fallback_key).cloned())
                    } else {
                        outputs
                            .get(&context_key)
                            .cloned()
                            .or_else(|| outputs.get(&fallback_key).cloned())
                    };
                    if let Some(value) = value {
                        inputs.insert(incoming.to_input_name.clone(), value);
                    }
                }

                let context = NodeEvaluationContext {
                    graph_id: self.graph_id.clone(),
                    graph_name: self.graph_name.clone(),
                    elapsed_seconds,
                    render_layout: render_context.as_ref().map(|ctx| ctx.layout.clone()),
                };
                tracing::trace!(
                    graph_id,
                    node_id = %node.id,
                    node_type = %node.node_type.as_str(),
                    context_id,
                    input_names = ?inputs.keys().collect::<Vec<_>>(),
                    elapsed_seconds,
                    "evaluating node"
                );
                let evaluator = execution_state
                    .evaluators
                    .entry((node_index, context_id.to_owned()))
                    .or_insert_with(|| {
                        self.node_registry
                            .evaluator_for(node.node_type.as_str(), &node.parameters)
                            .expect("validated node type must have runtime evaluator")
                    });
                let evaluation = match evaluate_node(&context, evaluator.as_mut(), &inputs) {
                    Ok(evaluation) => evaluation,
                    Err(error) => {
                        events.node_diagnostics(
                            graph_id.to_owned(),
                            node.id.clone(),
                            vec![NodeDiagnostic {
                                severity: NodeDiagnosticSeverity::Error,
                                code: Some("runtime_evaluation_failed".to_owned()),
                                message: error.to_string(),
                            }],
                        );
                        return Err(error).with_context(|| {
                            format!(
                                "evaluate node {} ({}) in context {}",
                                node.id,
                                node.node_type.as_str(),
                                context_id
                            )
                        });
                    }
                };
                if !evaluation.diagnostics.is_empty() {
                    events.node_diagnostics(
                        graph_id.to_owned(),
                        node.id.clone(),
                        evaluation.diagnostics.clone(),
                    );
                }
                let node_updates = evaluation
                    .frontend_updates
                    .into_iter()
                    .filter(|update| runtime_update_name_allowed(node, &update.name))
                    .map(|update| NodeRuntimeValue {
                        name: update.name,
                        value: update.value,
                    })
                    .collect::<Vec<_>>();
                let update_key = (node_index, context_id.to_owned());
                let should_emit_runtime_update = node_updates.is_empty()
                    || should_emit_runtime_update(
                        execution_state
                            .last_runtime_update_instants
                            .get(&update_key)
                            .copied(),
                        tick_started_at,
                        RUNTIME_UPDATE_MIN_INTERVAL,
                    );
                if !node_updates.is_empty() && should_emit_runtime_update {
                    tracing::trace!(
                        graph_id,
                        node_id = %node.id,
                        node_type = %node.node_type.as_str(),
                        context_id,
                        runtime_update_names = ?node_updates.iter().map(|value| value.name.as_str()).collect::<Vec<_>>(),
                        "emitting node runtime updates"
                    );
                    execution_state
                        .last_runtime_update_instants
                        .insert(update_key, tick_started_at);
                    events.node_runtime_update(graph_id.to_owned(), node.id.clone(), node_updates);
                }

                let output_names = evaluation.outputs.keys().cloned().collect::<Vec<_>>();
                tracing::trace!(
                    graph_id,
                    node_id = %node.id,
                    node_type = %node.node_type.as_str(),
                    context_id,
                    output_names = ?output_names,
                    "node evaluation completed"
                );

                for (name, value) in evaluation.outputs {
                    outputs.insert((node_index, context_id.to_owned(), name), value);
                }
            }
        }

        execution_state.previous_outputs = outputs;
        Ok(())
    }
}

/// Returns the render contexts in which a node should be evaluated for the current tick.
///
/// Nodes without explicit render contexts are evaluated once in the default context.
fn evaluation_contexts_for_node(
    contexts: &[crate::services::runtime::types::RenderContext],
) -> Vec<Option<crate::services::runtime::types::RenderContext>> {
    if contexts.is_empty() {
        vec![None]
    } else {
        contexts.iter().cloned().map(Some).collect::<Vec<_>>()
    }
}

/// Seeds previous-tick outputs for delay nodes when they have not produced a value yet.
///
/// Delay nodes need an initial value so feedback cycles can evaluate on the first tick. The
/// seeded value follows the node's configured `initial_type` parameter, using render-layout shape
/// information when available.
fn initialize_delay_previous_outputs(
    graph: &CompiledGraph,
    execution_state: &mut GraphExecutionState,
) {
    const DEFAULT_CONTEXT_ID: &str = "__default__";

    for (node_index, node) in graph.nodes.iter().enumerate() {
        if node.node_type.as_str() != shared::NodeTypeId::DELAY {
            continue;
        }

        let render_contexts = if graph.render_contexts_by_node[node_index].is_empty() {
            vec![None]
        } else {
            graph.render_contexts_by_node[node_index]
                .iter()
                .cloned()
                .map(Some)
                .collect::<Vec<_>>()
        };

        for render_context in render_contexts {
            let initial_type_id = node
                .parameters
                .get("initial_type")
                .and_then(|value| value.as_str());
            let (context_id, value) = match render_context {
                Some(context) => (
                    context.id,
                    seeded_initial_value_for_layout(initial_type_id, Some(&context.layout)),
                ),
                None => (
                    DEFAULT_CONTEXT_ID.to_owned(),
                    seeded_initial_value_for_layout(initial_type_id, None),
                ),
            };

            execution_state
                .previous_outputs
                .entry((node_index, context_id, "value".to_owned()))
                .or_insert(value);
        }
    }
}

/// Evaluates a runtime node evaluator with the prepared input map.
fn evaluate_node(
    context: &NodeEvaluationContext,
    evaluator: &mut dyn RuntimeNodeEvaluator,
    inputs: &HashMap<String, InputValue>,
) -> anyhow::Result<NodeEvaluation> {
    evaluator.evaluate(context, inputs)
}

/// Returns whether a frontend runtime update name is allowed for a compiled node.
///
/// Display nodes are allowed to emit `value*` updates when `value` is part of the declared
/// runtime-update schema so indexed display outputs can still be surfaced in the frontend.
fn runtime_update_name_allowed(
    node: &crate::services::runtime::types::CompiledNode,
    name: &str,
) -> bool {
    node.allowed_runtime_update_names.contains(name)
        || (node.node_type.as_str() == shared::NodeTypeId::DISPLAY
            && node.allowed_runtime_update_names.contains("value")
            && name.starts_with("value"))
}

/// Returns whether a runtime update should be emitted at `now`.
///
/// Updates are rate-limited per node and render context using wall-clock time so manual stepping
/// and simulated-time jumps do not affect preview cadence.
fn should_emit_runtime_update(
    last_emitted_at: Option<Instant>,
    now: Instant,
    min_interval: Duration,
) -> bool {
    match last_emitted_at {
        None => true,
        Some(last) => now.duration_since(last) >= min_interval,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::time::{Duration, Instant};

    use anyhow::Context;
    use shared::{GraphDocument, InputValue};

    use crate::node_runtime::{NodeEvaluationContext, build_node_registry};
    use crate::services::runtime::compiler::compile_graph_document;
    use crate::services::runtime::executor::{evaluate_node, initialize_delay_previous_outputs};
    use crate::services::runtime::types::{GraphExecutionState, RuntimeEventPublisher};

    struct NoopEvents;

    impl RuntimeEventPublisher for NoopEvents {
        /// Ignores runtime-status broadcasts in executor tests.
        fn runtime_statuses_changed(&self, _statuses: Vec<shared::GraphRuntimeStatus>) {}

        /// Ignores node runtime updates in executor tests.
        fn node_runtime_update(
            &self,
            _graph_id: String,
            _node_id: String,
            _values: Vec<shared::NodeRuntimeValue>,
        ) {
        }

        /// Ignores diagnostics emitted during executor tests.
        fn node_diagnostics(
            &self,
            _graph_id: String,
            _node_id: String,
            _diagnostics: Vec<shared::NodeDiagnostic>,
        ) {
        }
    }

    fn sample_runtime_graph() -> GraphDocument {
        serde_json::from_value(serde_json::json!({
            "metadata": {
                "id": "runtime-sample",
                "name": "runtime sample",
                "execution_frequency_hz": 60
            },
            "nodes": [
                {
                    "id": "signal_1",
                    "metadata": { "name": "signal_1" },
                    "node_type": "inputs.signal_generator",
                    "parameters": [
                        { "name": "waveform", "value": "sinus" },
                        { "name": "frequency", "value": 0.5 },
                        { "name": "amplitude", "value": 1.0 },
                        { "name": "phase", "value": 0.0 },
                        { "name": "offset", "value": 0.0 }
                    ]
                },
                {
                    "id": "scale_1",
                    "metadata": { "name": "scale_1" },
                    "node_type": "math.multiply"
                },
                {
                    "id": "solid_1",
                    "metadata": { "name": "solid_1" },
                    "node_type": "generators.solid_frame",
                    "parameters": [
                        { "name": "layout", "value": { "id": "strip-16", "pixel_count": 16, "width": 16, "height": 1 } },
                        { "name": "color", "value": { "r": 1.0, "g": 0.55, "b": 0.18, "a": 1.0 } }
                    ]
                },
                {
                    "id": "brightness_1",
                    "metadata": { "name": "brightness_1" },
                    "node_type": "frame_operations.frame_brightness"
                },
                {
                    "id": "display_1",
                    "metadata": { "name": "display_1" },
                    "node_type": "debug.wled_dummy_display",
                    "parameters": [
                        { "name": "width", "value": 4 },
                        { "name": "height", "value": 4 }
                    ]
                }
            ],
            "edges": [
                {
                    "from_node_id": "signal_1",
                    "from_output_name": "value",
                    "to_node_id": "scale_1",
                    "to_input_name": "a"
                },
                {
                    "from_node_id": "signal_1",
                    "from_output_name": "value",
                    "to_node_id": "scale_1",
                    "to_input_name": "b"
                },
                {
                    "from_node_id": "solid_1",
                    "from_output_name": "frame",
                    "to_node_id": "brightness_1",
                    "to_input_name": "frame"
                },
                {
                    "from_node_id": "scale_1",
                    "from_output_name": "product",
                    "to_node_id": "brightness_1",
                    "to_input_name": "factor"
                },
                {
                    "from_node_id": "brightness_1",
                    "from_output_name": "frame",
                    "to_node_id": "display_1",
                    "to_input_name": "value"
                }
            ]
        }))
        .expect("parse sample runtime graph")
    }

    /// Tests that the sample runtime graph executes a single tick without errors.
    #[test]
    fn sample_runtime_graph_executes_one_tick() {
        let document = sample_runtime_graph();
        let node_registry = build_node_registry().expect("build node registry");
        let mut graph =
            compile_graph_document(document, node_registry).expect("compile graph document");
        let mut execution_state = GraphExecutionState::default();

        graph
            .execute_tick("runtime-sample", &NoopEvents, 0.0, &mut execution_state)
            .expect("execute sample graph tick");
    }

    #[test]
    fn binary_select_graph_executes_one_tick() {
        let document: GraphDocument = serde_json::from_value(serde_json::json!({
            "metadata": {
                "id": "binary-select-sample",
                "name": "binary select sample",
                "execution_frequency_hz": 60
            },
            "nodes": [
                {
                    "id": "selector",
                    "metadata": { "name": "selector" },
                    "node_type": "inputs.float_constant",
                    "parameters": [{ "name": "value", "value": 1.0 }]
                },
                {
                    "id": "a",
                    "metadata": { "name": "a" },
                    "node_type": "inputs.float_constant",
                    "parameters": [{ "name": "value", "value": 2.0 }]
                },
                {
                    "id": "b",
                    "metadata": { "name": "b" },
                    "node_type": "inputs.float_constant",
                    "parameters": [{ "name": "value", "value": 7.0 }]
                },
                {
                    "id": "select",
                    "metadata": { "name": "select" },
                    "node_type": "math.binary_select"
                }
            ],
            "edges": [
                {
                    "from_node_id": "selector",
                    "from_output_name": "value",
                    "to_node_id": "select",
                    "to_input_name": "selector"
                },
                {
                    "from_node_id": "a",
                    "from_output_name": "value",
                    "to_node_id": "select",
                    "to_input_name": "a"
                },
                {
                    "from_node_id": "b",
                    "from_output_name": "value",
                    "to_node_id": "select",
                    "to_input_name": "b"
                }
            ]
        }))
        .expect("parse binary select graph");

        let node_registry = build_node_registry().expect("build node registry");
        let mut graph =
            compile_graph_document(document, node_registry).expect("compile binary select graph");
        let mut execution_state = GraphExecutionState::default();

        graph
            .execute_tick(
                "binary-select-sample",
                &NoopEvents,
                0.0,
                &mut execution_state,
            )
            .expect("execute binary select graph tick");

        let selected = execution_state
            .previous_outputs
            .get(&(3usize, "__default__".to_owned(), "value".to_owned()))
            .cloned()
            .expect("binary select output");
        assert_eq!(selected, InputValue::Float(7.0));
    }

    /// Measures the average tick time for the sample runtime graph.
    #[test]
    fn sample_runtime_graph_tick_timing() {
        let document = sample_runtime_graph();
        let node_registry = build_node_registry().expect("build node registry");
        let mut graph =
            compile_graph_document(document, node_registry).expect("compile graph document");
        let mut execution_state = GraphExecutionState::default();

        graph
            .execute_tick("runtime-sample", &NoopEvents, 0.0, &mut execution_state)
            .expect("warm up sample graph tick");

        let iterations = 200usize;
        let start = Instant::now();
        for index in 0..iterations {
            graph
                .execute_tick(
                    "runtime-sample",
                    &NoopEvents,
                    index as f64 / 60.0,
                    &mut execution_state,
                )
                .expect("execute sample graph tick");
        }
        let total = start.elapsed();
        let average = Duration::from_secs_f64(total.as_secs_f64() / iterations as f64);
        tracing::info!(
            iterations,
            total_millis = total.as_secs_f64() * 1000.0,
            average_millis = average.as_secs_f64() * 1000.0,
            "sample graph tick timing"
        );
    }

    /// Records a per-node timing breakdown for the sample runtime graph.
    #[test]
    fn sample_runtime_graph_tick_breakdown() {
        const DEFAULT_CONTEXT_ID: &str = "__default__";

        let document = sample_runtime_graph();
        let node_registry = build_node_registry().expect("build node registry");
        let mut graph =
            compile_graph_document(document, node_registry).expect("compile graph document");
        let mut execution_state = GraphExecutionState::default();

        let mut totals = HashMap::<(String, String, String), Duration>::new();
        let mut counts = HashMap::<(String, String, String), usize>::new();
        let iterations = 200usize;

        for index in 0..iterations {
            let elapsed_seconds = index as f64 / 60.0;
            let previous_outputs = execution_state.previous_outputs.clone();
            let mut outputs = HashMap::<(usize, String, String), InputValue>::new();

            let topological_order = graph.topological_order.clone();
            for node_index in topological_order {
                let render_contexts = if graph.render_contexts_by_node[node_index].is_empty() {
                    vec![None]
                } else {
                    graph.render_contexts_by_node[node_index]
                        .iter()
                        .cloned()
                        .map(Some)
                        .collect::<Vec<_>>()
                };
                let incoming_edges = graph.incoming_edges_by_node[node_index].clone();
                let node = &mut graph.nodes[node_index];

                for render_context in render_contexts {
                    let context_id = render_context
                        .as_ref()
                        .map(|ctx| ctx.id.as_str())
                        .unwrap_or(DEFAULT_CONTEXT_ID);
                    let mut inputs = node.input_defaults.clone();

                    for incoming in &incoming_edges {
                        let context_key = (
                            incoming.from_node_index,
                            context_id.to_owned(),
                            incoming.from_output_name.clone(),
                        );
                        let fallback_key = (
                            incoming.from_node_index,
                            DEFAULT_CONTEXT_ID.to_owned(),
                            incoming.from_output_name.clone(),
                        );
                        let value = if incoming.use_previous_tick {
                            previous_outputs
                                .get(&context_key)
                                .cloned()
                                .or_else(|| previous_outputs.get(&fallback_key).cloned())
                        } else {
                            outputs
                                .get(&context_key)
                                .cloned()
                                .or_else(|| outputs.get(&fallback_key).cloned())
                        };
                        if let Some(value) = value {
                            inputs.insert(incoming.to_input_name.clone(), value);
                        }
                    }

                    let context = NodeEvaluationContext {
                        graph_id: graph.graph_id.clone(),
                        graph_name: graph.graph_name.clone(),
                        elapsed_seconds,
                        render_layout: render_context.as_ref().map(|ctx| ctx.layout.clone()),
                    };
                    let evaluator = execution_state
                        .evaluators
                        .entry((node_index, context_id.to_owned()))
                        .or_insert_with(|| {
                            graph
                                .node_registry
                                .evaluator_for(node.node_type.as_str(), &node.parameters)
                                .expect("validated node type must have runtime evaluator")
                        });

                    let started = Instant::now();
                    let evaluation = evaluate_node(&context, evaluator.as_mut(), &inputs)
                        .with_context(|| {
                            format!(
                                "evaluate node {} ({}) in context {}",
                                node.id,
                                node.node_type.as_str(),
                                context_id
                            )
                        })
                        .expect("evaluate sample graph node");
                    let elapsed = started.elapsed();

                    let key = (
                        node.id.clone(),
                        node.node_type.as_str().to_owned(),
                        context_id.to_owned(),
                    );
                    *totals.entry(key.clone()).or_default() += elapsed;
                    *counts.entry(key).or_default() += 1;

                    for (name, value) in evaluation.outputs {
                        outputs.insert((node_index, context_id.to_owned(), name), value);
                    }
                }
            }
            execution_state.previous_outputs = outputs;
        }

        let mut rows = totals
            .into_iter()
            .map(|((node_id, node_type, context_id), total)| {
                let count = counts
                    .get(&(node_id.clone(), node_type.clone(), context_id.clone()))
                    .copied()
                    .unwrap_or(1);
                let avg = Duration::from_secs_f64(total.as_secs_f64() / count as f64);
                (total, avg, count, node_id, node_type, context_id)
            })
            .collect::<Vec<_>>();
        rows.sort_by(|lhs, rhs| rhs.0.cmp(&lhs.0));

        let total_time = rows
            .iter()
            .fold(Duration::ZERO, |acc, (total, _, _, _, _, _)| acc + *total);
        tracing::info!(iterations, "sample graph per-node breakdown");
        for (total, avg, count, node_id, node_type, context_id) in rows.iter().take(20) {
            let pct = if total_time.is_zero() {
                0.0
            } else {
                total.as_secs_f64() * 100.0 / total_time.as_secs_f64()
            };
            tracing::info!(
                node_id,
                node_type,
                context_id,
                count,
                total_millis = total.as_secs_f64() * 1000.0,
                average_millis = avg.as_secs_f64() * 1000.0,
                share_percent = pct,
                "sample graph node timing"
            );
        }
        tracing::info!(
            aggregate_millis = total_time.as_secs_f64() * 1000.0,
            "sample graph timing aggregate"
        );
    }

    #[test]
    /// Tests that a delay node can seed a feedback cycle without deadlocking execution.
    fn delay_node_allows_feedback_cycle() {
        let document: GraphDocument = serde_json::from_value(serde_json::json!({
            "metadata": {
                "id": "delay-cycle",
                "name": "delay cycle",
                "execution_frequency_hz": 60
            },
            "nodes": [
                {
                    "id": "constant_1",
                    "metadata": { "name": "constant_1" },
                    "node_type": "inputs.float_constant",
                    "parameters": [{ "name": "value", "value": 1.0 }]
                },
                {
                    "id": "add_1",
                    "metadata": { "name": "add_1" },
                    "node_type": "math.add"
                },
                {
                    "id": "delay_1",
                    "metadata": { "name": "delay_1" },
                    "node_type": "temporal_filters.delay",
                    "parameters": [{ "name": "ticks", "value": 1 }]
                }
            ],
            "edges": [
                {
                    "from_node_id": "constant_1",
                    "from_output_name": "value",
                    "to_node_id": "add_1",
                    "to_input_name": "a"
                },
                {
                    "from_node_id": "delay_1",
                    "from_output_name": "value",
                    "to_node_id": "add_1",
                    "to_input_name": "b"
                },
                {
                    "from_node_id": "add_1",
                    "from_output_name": "sum",
                    "to_node_id": "delay_1",
                    "to_input_name": "value"
                }
            ]
        }))
        .expect("parse delay cycle graph");

        let node_registry = build_node_registry().expect("build node registry");
        let mut graph =
            compile_graph_document(document, node_registry).expect("compile delay cycle graph");
        let mut execution_state = GraphExecutionState::default();

        graph
            .execute_tick("delay-cycle", &NoopEvents, 0.0, &mut execution_state)
            .expect("execute first delay cycle tick");
        let first = execution_state
            .previous_outputs
            .get(&(1usize, "__default__".to_owned(), "sum".to_owned()))
            .cloned()
            .expect("first add output");
        assert_eq!(first, InputValue::Float(1.0));

        graph
            .execute_tick("delay-cycle", &NoopEvents, 1.0 / 60.0, &mut execution_state)
            .expect("execute second delay cycle tick");
        let second = execution_state
            .previous_outputs
            .get(&(1usize, "__default__".to_owned(), "sum".to_owned()))
            .cloned()
            .expect("second add output");
        assert_eq!(second, InputValue::Float(1.0));

        graph
            .execute_tick("delay-cycle", &NoopEvents, 2.0 / 60.0, &mut execution_state)
            .expect("execute third delay cycle tick");
        let third = execution_state
            .previous_outputs
            .get(&(1usize, "__default__".to_owned(), "sum".to_owned()))
            .cloned()
            .expect("third add output");
        assert_eq!(third, InputValue::Float(2.0));
    }

    #[test]
    /// Tests that delay nodes seed transparent frames for render-context outputs.
    fn delay_previous_output_initializes_to_transparent_frame_for_render_contexts() {
        let document: GraphDocument = serde_json::from_value(serde_json::json!({
            "metadata": {
                "id": "delay-frame-seed",
                "name": "delay frame seed",
                "execution_frequency_hz": 60
            },
            "nodes": [
                {
                    "id": "delay_1",
                    "metadata": { "name": "delay_1" },
                    "node_type": "temporal_filters.delay",
                    "parameters": [
                        { "name": "ticks", "value": 1 },
                        { "name": "initial_type", "value": "colorframe" }
                    ]
                },
                {
                    "id": "wled_1",
                    "metadata": { "name": "wled_1" },
                    "node_type": "outputs.wled_target",
                    "parameters": [
                        { "name": "target", "value": "dummy" },
                        { "name": "led_count", "value": 4 }
                    ]
                }
            ],
            "edges": [
                {
                    "from_node_id": "delay_1",
                    "from_output_name": "value",
                    "to_node_id": "wled_1",
                    "to_input_name": "value"
                }
            ]
        }))
        .expect("parse delay frame seed graph");
        let node_registry = build_node_registry().expect("build node registry");
        let graph = compile_graph_document(document, node_registry)
            .expect("compile delay frame seed graph");
        let mut execution_state = GraphExecutionState::default();

        initialize_delay_previous_outputs(&graph, &mut execution_state);

        let seeded_frame = execution_state
            .previous_outputs
            .values()
            .find_map(|value| match value {
                InputValue::ColorFrame(frame) => Some(frame),
                _ => None,
            })
            .expect("seeded transparent delay frame");

        assert!(
            seeded_frame.pixels.iter().all(|pixel| {
                pixel.r == 0.0 && pixel.g == 0.0 && pixel.b == 0.0 && pixel.a == 0.0
            })
        );
    }

    #[test]
    fn delay_previous_output_honors_tensor_initial_type_in_render_contexts() {
        let document: GraphDocument = serde_json::from_value(serde_json::json!({
            "metadata": {
                "id": "delay-tensor-seed",
                "name": "delay tensor seed",
                "execution_frequency_hz": 60
            },
            "nodes": [
                {
                    "id": "delay_1",
                    "metadata": { "name": "delay_1" },
                    "node_type": "temporal_filters.delay",
                    "parameters": [
                        { "name": "ticks", "value": 1 },
                        { "name": "initial_type", "value": "tensor" }
                    ]
                },
                {
                    "id": "wled_1",
                    "metadata": { "name": "wled_1" },
                    "node_type": "outputs.wled_target",
                    "parameters": [
                        { "name": "target", "value": "dummy" },
                        { "name": "led_count", "value": 4 }
                    ]
                }
            ],
            "edges": [
                {
                    "from_node_id": "delay_1",
                    "from_output_name": "value",
                    "to_node_id": "wled_1",
                    "to_input_name": "value"
                }
            ]
        }))
        .expect("parse delay tensor seed graph");
        let node_registry = build_node_registry().expect("build node registry");
        let graph = compile_graph_document(document, node_registry)
            .expect("compile delay tensor seed graph");
        let mut execution_state = GraphExecutionState::default();

        initialize_delay_previous_outputs(&graph, &mut execution_state);

        let seeded_tensor = execution_state
            .previous_outputs
            .values()
            .find_map(|value| match value {
                InputValue::FloatTensor(tensor) => Some(tensor),
                _ => None,
            })
            .expect("seeded tensor delay output");

        assert_eq!(seeded_tensor.shape, vec![4]);
        assert_eq!(seeded_tensor.values, vec![0.0; 4]);
    }
}
